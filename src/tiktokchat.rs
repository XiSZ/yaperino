//! TikTok live chat, read only (no account needed), following Moltorino's
//! TikTokChatServer.cpp / TikTokProtocol.cpp:
//! 1. a guest `ttwid` cookie from `POST https://www.tiktok.com/ttwid/check/`
//!    (then `/ttwid/register/` when asked),
//! 2. the room id from the live check (`api-live/user/room`),
//! 3. a websocket to `webcast-ws.tiktok.com/webcast/im/ws_proxy/ws_reuse_supplement/`,
//!    an "im_enter_room" frame, a "hb" heartbeat every 10 s, and "ack" frames when asked,
//! 4. protobuf frames (payload often gzip): messages of type `WebcastChatMessage`
//!    carry the user (3 = nickname, 38 = handle) and the text (field 3).
//! Sending needs TikTok's signed requests, which this app does not rebuild.

use crate::chat::{clock, ChatLine, Event};
use crate::platforms::Platform;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tungstenite::client::IntoClientRequest;
use tungstenite::http::HeaderValue;
use tungstenite::Message;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";
pub const WS_URL: &str = "wss://webcast-ws.tiktok.com/webcast/im/ws_proxy/ws_reuse_supplement/";
pub const HEARTBEAT_SECS: u64 = 10;

// ------------------------------------------------------------------ protobuf

fn varint(data: &[u8], pos: &mut usize) -> Option<u64> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let b = *data.get(*pos)?;
        *pos += 1;
        v |= u64::from(b & 0x7f) << shift;
        if b & 0x80 == 0 {
            return Some(v);
        }
    }
    None
}

#[derive(Debug, Clone)]
pub struct Field<'a> {
    pub number: u32,
    pub value: u64,
    pub bytes: &'a [u8],
}

/// All fields of one protobuf message (unknown wire types end the parse).
pub fn fields(data: &[u8]) -> Vec<Field<'_>> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < data.len() {
        let Some(tag) = varint(data, &mut pos) else {
            break;
        };
        let (number, wire) = ((tag >> 3) as u32, tag & 7);
        match wire {
            0 => {
                let Some(v) = varint(data, &mut pos) else {
                    break;
                };
                out.push(Field {
                    number,
                    value: v,
                    bytes: &[],
                });
            }
            1 | 2 | 5 => {
                let len = match wire {
                    2 => match varint(data, &mut pos) {
                        Some(l) => l as usize,
                        None => break,
                    },
                    1 => 8,
                    _ => 4,
                };
                if pos + len > data.len() {
                    break;
                }
                out.push(Field {
                    number,
                    value: 0,
                    bytes: &data[pos..pos + len],
                });
                pos += len;
            }
            _ => break,
        }
    }
    out
}

fn get<'a>(f: &[Field<'a>], n: u32) -> Option<&'a [u8]> {
    f.iter().find(|x| x.number == n).map(|x| x.bytes)
}

fn num(f: &[Field<'_>], n: u32) -> u64 {
    f.iter()
        .find(|x| x.number == n)
        .map(|x| x.value)
        .unwrap_or(0)
}

fn text(f: &[Field<'_>], n: u32) -> String {
    get(f, n)
        .map(|b| String::from_utf8_lossy(b).to_string())
        .unwrap_or_default()
}

fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return;
        }
        out.push(b | 0x80);
    }
}

fn integer(out: &mut Vec<u8>, field: u32, v: u64) {
    put_varint(out, u64::from(field) << 3);
    put_varint(out, v);
}

fn blob(out: &mut Vec<u8>, field: u32, v: &[u8]) {
    put_varint(out, (u64::from(field) << 3) | 2);
    put_varint(out, v.len() as u64);
    out.extend_from_slice(v);
}

/// The outer push frame: 2 = log id, 6 = "pb", 7 = kind, 8 = payload.
pub fn frame(kind: &str, payload: &[u8], log_id: u64) -> Vec<u8> {
    let mut out = Vec::new();
    integer(&mut out, 2, log_id);
    blob(&mut out, 6, b"pb");
    blob(&mut out, 7, kind.as_bytes());
    blob(&mut out, 8, payload);
    out
}

pub fn heartbeat(room: u64) -> Vec<u8> {
    let mut p = Vec::new();
    integer(&mut p, 1, room);
    frame("hb", &p, 0)
}

pub fn enter_room(room: u64) -> Vec<u8> {
    let mut p = Vec::new();
    integer(&mut p, 1, room);
    integer(&mut p, 4, 12);
    blob(&mut p, 5, b"audience");
    blob(&mut p, 9, b"0");
    frame("im_enter_room", &p, 0)
}

/// One decoded frame: chat lines (name, handle, text) and an ack to send back.
#[derive(Debug, Default)]
pub struct Batch {
    pub chats: Vec<(String, String, String)>,
    pub ack: Option<Vec<u8>>,
}

pub fn decode(data: &[u8]) -> Batch {
    let mut batch = Batch::default();
    let outer = fields(data);
    let kind = text(&outer, 7);
    if kind != "msg" && kind != "im_enter_room_resp" {
        return batch;
    }
    let mut payload = get(&outer, 8).unwrap_or(&[]).to_vec();
    if payload.len() >= 2 && payload[0] == 0x1f && payload[1] == 0x8b {
        let mut inflated = Vec::new();
        let ok = flate2::read::GzDecoder::new(&payload[..])
            .take(4 * 1024 * 1024)
            .read_to_end(&mut inflated)
            .is_ok();
        if !ok {
            return batch;
        }
        payload = inflated;
    }
    let response = fields(&payload);
    if num(&response, 9) != 0 {
        batch.ack = Some(frame(
            "ack",
            get(&response, 5).unwrap_or(&[]),
            num(&outer, 2),
        ));
    }
    for m in response.iter().filter(|f| f.number == 1) {
        let wrapper = fields(m.bytes);
        if text(&wrapper, 1) != "WebcastChatMessage" {
            continue;
        }
        let p = fields(get(&wrapper, 2).unwrap_or(&[]));
        let user = fields(get(&p, 2).unwrap_or(&[]));
        let name = text(&user, 3).trim().to_string();
        let handle = text(&user, 38);
        let content = text(&p, 3);
        if !content.is_empty() {
            batch.chats.push((
                if name.is_empty() {
                    handle.clone()
                } else {
                    name
                },
                handle,
                content,
            ));
        }
    }
    batch
}

// ------------------------------------------------------------------ connection

/// The guest `ttwid` cookie (`ttwid=...`), from the Set-Cookie of check/register.
pub fn guest_cookie() -> Result<String, String> {
    let options = r#"{"aid":1988,"service":"www.tiktok.com","union":false,"unionHost":"","needFid":false,"fid":""}"#;
    let check = r#"{"aid":1988,"service":"www.tiktok.com","union":false,"unionHost":"","needFid":false,"fid":"","migrate_priority":0}"#;
    let headers = [
        ("Content-Type", "application/json"),
        ("Origin", "https://www.tiktok.com"),
        ("Referer", "https://www.tiktok.com/"),
    ];
    let find = |head: &str| -> Option<String> {
        head.lines()
            .filter(|l| l.to_ascii_lowercase().starts_with("set-cookie:"))
            .filter_map(|l| l.split_once(':').map(|x| x.1.trim().to_string()))
            .find(|c| c.starts_with("ttwid="))
            .and_then(|c| c.split(';').next().map(String::from))
    };
    let (status, head, body) = crate::platforms::curl_full(
        "POST",
        "https://www.tiktok.com/ttwid/check/",
        &headers,
        Some(check),
        true,
    )
    .unwrap_or_else(|| Err("curl.exe not found".into()))?;
    if let Some(c) = find(&head) {
        return Ok(c);
    }
    let v: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    let code = v.get("status_code").and_then(|c| c.as_i64()).unwrap_or(-1);
    if status != 200 || code < 0 {
        return Err(format!("TikTok guest cookie: {status}"));
    }
    let mut reg: serde_json::Value = serde_json::from_str(options).unwrap_or_default();
    if let Some(m) = v.get("migrate_info") {
        reg["migrate_info"] = m.clone();
    }
    let (_, head, _) = crate::platforms::curl_full(
        "POST",
        "https://www.tiktok.com/ttwid/register/",
        &headers,
        Some(&reg.to_string()),
        true,
    )
    .unwrap_or_else(|| Err("curl.exe not found".into()))?;
    find(&head).ok_or_else(|| "TikTok gave no guest cookie".to_string())
}

pub fn socket_url(room: &str) -> String {
    let q = [
        ("version_code", "180800"),
        ("device_platform", "web"),
        ("cookie_enabled", "true"),
        ("screen_width", "1920"),
        ("screen_height", "1080"),
        ("browser_language", "en-US"),
        ("browser_platform", "Win32"),
        ("browser_name", "Mozilla"),
        ("browser_version", "5.0 (Windows)"),
        ("browser_online", "true"),
        ("tz_name", "Europe/Berlin"),
        ("app_name", "tiktok_web"),
        ("sup_ws_ds_opt", "1"),
        ("update_version_code", "2.0.0"),
        ("compress", "gzip"),
        ("webcast_language", "en"),
        ("ws_direct", "1"),
        ("aid", "1988"),
        ("live_id", "12"),
        ("app_language", "en"),
        ("client_enter", "1"),
        ("room_id", room),
        ("identity", "audience"),
        ("history_comment_count", "6"),
        ("last_rtt", "150.000"),
        ("heartbeat_duration", "10000"),
        ("resp_content_type", "protobuf"),
        ("did_rule", "3"),
    ];
    let query: Vec<String> = q
        .iter()
        .map(|(k, v)| {
            format!(
                "{k}={}",
                v.replace(' ', "%20")
                    .replace('(', "%28")
                    .replace(')', "%29")
            )
        })
        .collect();
    format!("{WS_URL}?{}", query.join("&"))
}

/// The live room id of a TikTok user, or None when not live.
pub fn room_id(user: &str) -> Result<Option<String>, String> {
    let url = format!(
        "{}{}",
        crate::platforms::TIKTOK_ROOM,
        user.trim_start_matches('@')
    );
    let (status, body) = crate::platforms::curl_full("GET", &url, &[], None, false)
        .map(|r| r.map(|(s, _, b)| (s, b)))
        .unwrap_or_else(|| Err("curl.exe not found".into()))?;
    if status != 200 {
        return Err(format!("TikTok answered {status}"));
    }
    let v: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let user = v.pointer("/data/user").cloned().unwrap_or_default();
    let live = user.get("status").and_then(|s| s.as_i64()) == Some(2);
    let id = user
        .get("roomId")
        .and_then(|r| r.as_str())
        .unwrap_or("")
        .to_string();
    Ok((live && !id.is_empty() && id != "0").then_some(id))
}

pub struct Reader {
    stop: Arc<AtomicBool>,
}

impl Drop for Reader {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

pub fn start(group: String, user: String, tx: Sender<Event>, ctx: egui::Context) -> Reader {
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    std::thread::spawn(move || {
        let emit = |e: Event| {
            let _ = tx.send(e);
            ctx.request_repaint();
        };
        // the last message shown: checking again every minute stays quiet until
        // something changes (not live -> live, or a different problem)
        let mut last: Option<String> = None;
        while !stop2.load(Ordering::Relaxed) {
            let r = match room_id(&user) {
                Ok(Some(room)) => {
                    last = None;
                    session(&group, &room, &stop2, &emit)
                }
                Ok(None) => Err(format!("@{user} is not live on TikTok")),
                Err(e) => Err(e),
            };
            if let Err(e) = r {
                if last.as_deref() != Some(e.as_str()) {
                    emit(Event::Line(
                        group.clone(),
                        ChatLine::System(format!("TikTok chat: {e}")),
                    ));
                    last = Some(e);
                }
            }
            // check again in a minute
            for _ in 0..60 {
                if stop2.load(Ordering::Relaxed) {
                    return;
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    });
    Reader { stop }
}

fn session(
    group: &str,
    room: &str,
    stop: &AtomicBool,
    emit: &impl Fn(Event),
) -> Result<(), String> {
    let cookie = guest_cookie()?;
    let room_num: u64 = room.parse().map_err(|_| "bad room id".to_string())?;
    let mut req = socket_url(room)
        .into_client_request()
        .map_err(|e| e.to_string())?;
    let h = req.headers_mut();
    for (k, v) in [
        ("User-Agent", UA),
        ("Origin", "https://www.tiktok.com"),
        ("Referer", "https://www.tiktok.com/"),
    ] {
        h.insert(k, HeaderValue::from_static(v));
    }
    h.insert(
        "Cookie",
        HeaderValue::from_str(&cookie).map_err(|e| e.to_string())?,
    );
    let (mut ws, _) = tungstenite::connect(req).map_err(|e| e.to_string())?;
    if let tungstenite::stream::MaybeTlsStream::Rustls(s) = ws.get_ref() {
        let _ = s
            .get_ref()
            .set_read_timeout(Some(Duration::from_millis(250)));
    }
    ws.send(Message::Binary(enter_room(room_num).into()))
        .map_err(|e| e.to_string())?;
    emit(Event::Line(
        group.to_string(),
        ChatLine::System("Joined TikTok chat (read only)".into()),
    ));
    let mut last_hb = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        if last_hb.elapsed() >= Duration::from_secs(HEARTBEAT_SECS) {
            ws.send(Message::Binary(heartbeat(room_num).into()))
                .map_err(|e| e.to_string())?;
            last_hb = Instant::now();
        }
        match ws.read() {
            Ok(Message::Binary(b)) => {
                let batch = decode(&b);
                if let Some(ack) = batch.ack {
                    let _ = ws.send(Message::Binary(ack.into()));
                }
                for (name, _handle, text) in batch.chats {
                    emit(Event::Line(
                        group.to_string(),
                        ChatLine::Msg {
                            time: clock(),
                            name,
                            color: None,
                            text: text.clone(),
                            action: false,
                            own: false,
                            emotes: Vec::new(),
                            badges: Vec::new(),
                            segs: vec![crate::emotes::Seg::Text(text)],
                            platform: Platform::TikTok,
                            msg_id: String::new(),
                            user_id: String::new(),
                        },
                    ));
                }
            }
            Ok(Message::Close(f)) => {
                return Err(f
                    .map(|f| f.reason.to_string())
                    .unwrap_or_else(|| "closed".into()))
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    let _ = ws.close(None);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a frame the way TikTok sends chat and decodes it again.
    #[test]
    fn decodes_a_chat_frame() {
        let mut user = Vec::new();
        blob(&mut user, 3, "Some Name".as_bytes());
        blob(&mut user, 38, b"somename");
        let mut chat = Vec::new();
        blob(&mut chat, 2, &user);
        blob(&mut chat, 3, "hello tiktok".as_bytes());
        let mut wrapper = Vec::new();
        blob(&mut wrapper, 1, b"WebcastChatMessage");
        blob(&mut wrapper, 2, &chat);
        let mut response = Vec::new();
        blob(&mut response, 1, &wrapper);
        blob(&mut response, 5, b"ext");
        integer(&mut response, 9, 1);
        // gzip it, as TikTok does
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        std::io::Write::write_all(&mut gz, &response).unwrap();
        let packed = gz.finish().unwrap();
        let data = frame("msg", &packed, 77);
        let b = decode(&data);
        assert_eq!(
            b.chats,
            vec![(
                "Some Name".to_string(),
                "somename".to_string(),
                "hello tiktok".to_string()
            )]
        );
        let ack = b.ack.expect("ack");
        let f = fields(&ack);
        assert_eq!(text(&f, 7), "ack");
        assert_eq!(num(&f, 2), 77);
    }

    #[test]
    fn heartbeat_and_enter_frames() {
        let hb = heartbeat(123);
        let f = fields(&hb);
        assert_eq!(text(&f, 7), "hb");
        assert_eq!(num(&fields(get(&f, 8).unwrap()), 1), 123);
        let er = enter_room(5);
        let e = fields(&er);
        assert_eq!(text(&e, 7), "im_enter_room");
    }

    /// Network: gets a guest cookie. `cargo test -- --ignored tiktok_guest_cookie --nocapture`
    #[test]
    #[ignore]
    fn tiktok_guest_cookie() {
        let c = guest_cookie().expect("cookie");
        assert!(c.starts_with("ttwid="));
        println!("guest cookie ok ({} chars)", c.len());
    }
}
