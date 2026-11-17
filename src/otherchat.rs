//! Kick and YouTube chat for a tab, next to its Twitch chat (read only here;
//! Kick sending goes through platforms::kick_send).
//!
//! - Kick: Pusher websocket `wss://ws-us2.pusher.com/app/32cbd69e4b950bf97679`,
//!   channel `chatrooms.{id}.v2`, event `App\Events\ChatMessageEvent`; emotes are
//!   `[emote:ID:NAME]` in the text (Moltorino ws/KickPusherManager.cpp, KickChatServer.cpp).
//! - YouTube: the popout chat page gives a continuation; then
//!   `youtubei/v1/live_chat/get_live_chat` is polled with it (no account needed).

use crate::chat::{clock, ChatLine, Event};
use crate::emotes::{Emote, Provider, Seg};
use crate::platforms::Platform;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tungstenite::Message;

pub const KICK_PUSHER: &str =
    "wss://ws-us2.pusher.com/app/32cbd69e4b950bf97679?protocol=7&client=js&version=8.4.0&flash=false";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36";

/// Stops its thread when dropped.
pub struct Reader {
    stop: Arc<AtomicBool>,
}

impl Drop for Reader {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn msg(
    platform: Platform,
    name: String,
    color: Option<[u8; 3]>,
    text: String,
    segs: Vec<Seg>,
) -> ChatLine {
    ChatLine::Msg {
        time: clock(),
        name,
        color,
        text,
        action: false,
        own: false,
        emotes: Vec::new(),
        badges: Vec::new(),
        segs,
        platform,
        msg_id: String::new(),
        user_id: String::new(),
        reply: None,
    }
}

fn parse_hex(c: &str) -> Option<[u8; 3]> {
    let h = c.strip_prefix('#')?;
    if h.len() < 6 {
        return None;
    }
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some([p(0)?, p(2)?, p(4)?])
}

// ------------------------------------------------------------------ Kick

/// "hi [emote:37226:KEKW]" -> text and Kick emote images.
pub fn kick_segments(content: &str) -> Vec<Seg> {
    let mut out = Vec::new();
    let mut rest = content;
    while let Some(i) = rest.find("[emote:") {
        let Some(j) = rest[i..].find(']') else { break };
        let inner = &rest[i + 7..i + j];
        if let Some((id, name)) = inner.split_once(':') {
            if i > 0 {
                out.push(Seg::Text(rest[..i].to_string()));
            }
            out.push(Seg::Emote(Emote {
                name: name.to_string(),
                url: format!("{}{id}/fullsize", crate::platforms::KICK_EMOTE),
                provider: Provider::Kick,
            }));
        } else {
            out.push(Seg::Text(rest[..i + j + 1].to_string()));
        }
        rest = &rest[i + j + 1..];
    }
    if !rest.is_empty() {
        out.push(Seg::Text(rest.to_string()));
    }
    out
}

pub fn kick(
    group: String,
    channel_id: u64,
    chatroom_id: u64,
    tx: Sender<Event>,
    ctx: egui::Context,
) -> Reader {
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    std::thread::spawn(move || {
        let emit = |e: Event| {
            let _ = tx.send(e);
            ctx.request_repaint();
        };
        while !stop2.load(Ordering::Relaxed) {
            // Kick says which server to use each time (Centrifugo now, Pusher before)
            let result = match crate::platforms::kick_realtime(channel_id) {
                Ok(crate::platforms::KickRealtime::Centrifugo(url, token)) => {
                    centrifugo_session(&group, &url, &token, chatroom_id, &stop2, &emit)
                }
                Ok(crate::platforms::KickRealtime::Pusher(url)) => {
                    pusher_session(&group, &url, chatroom_id, &stop2, &emit)
                }
                Err(_) => pusher_session(&group, KICK_PUSHER, chatroom_id, &stop2, &emit),
            };
            if let Err(e) = result {
                emit(Event::Line(
                    group.clone(),
                    ChatLine::System(format!("Kick chat disconnected: {e}")),
                ));
                for _ in 0..10 {
                    if stop2.load(Ordering::Relaxed) {
                        return;
                    }
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
        }
    });
    Reader { stop }
}

fn set_short_timeout(
    ws: &tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
) {
    if let tungstenite::stream::MaybeTlsStream::Rustls(s) = ws.get_ref() {
        let _ = s
            .get_ref()
            .set_read_timeout(Some(Duration::from_millis(250)));
    }
}

/// A Kick chat message event's data (a JSON string or object) -> a chat line.
fn kick_message(data: &Value) -> Option<ChatLine> {
    let parsed;
    let data = match data {
        Value::String(s) => {
            parsed = serde_json::from_str::<Value>(s).ok()?;
            &parsed
        }
        other => other,
    };
    let content = data
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let sender = data.get("sender").cloned().unwrap_or(Value::Null);
    let name = sender
        .get("username")
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_string();
    let color = sender
        .get("identity")
        .and_then(|i| i.get("color"))
        .and_then(Value::as_str)
        .and_then(parse_hex);
    let segs = kick_segments(&content);
    Some(msg(Platform::Kick, name, color, content, segs))
}

fn is_chat_event(e: &str) -> bool {
    e == "App\\Events\\ChatMessageEvent" || e.ends_with("ChatMessageEvent")
}

/// Kick's Centrifugo chat (Moltorino KickCentrifugoManager.cpp): connect with the
/// guest token, subscribe to `chatrooms.{id}.v2`, read `push.pub.data`.
fn centrifugo_session(
    group: &str,
    url: &str,
    token: &str,
    chatroom_id: u64,
    stop: &AtomicBool,
    emit: &impl Fn(Event),
) -> Result<(), String> {
    let (mut ws, _) = tungstenite::connect(url).map_err(|e| e.to_string())?;
    set_short_timeout(&ws);
    let connect = serde_json::json!({"connect": {"token": token, "name": "js"}, "id": 1});
    ws.send(Message::Text(connect.to_string().into()))
        .map_err(|e| e.to_string())?;
    let channel = format!("chatrooms.{chatroom_id}.v2");
    while !stop.load(Ordering::Relaxed) {
        match ws.read() {
            Ok(Message::Text(t)) => {
                for line in t.split('\n').filter(|l| !l.trim().is_empty()) {
                    let Ok(v) = serde_json::from_str::<Value>(line) else {
                        continue;
                    };
                    if v.as_object().is_some_and(|o| o.is_empty()) {
                        // ping: answer with pong
                        let _ = ws.send(Message::Text("{}".into()));
                        continue;
                    }
                    if let Some(err) = v.get("error") {
                        return Err(err
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or("error")
                            .to_string());
                    }
                    match v.get("id").and_then(Value::as_u64) {
                        Some(1) => {
                            let sub =
                                serde_json::json!({"subscribe": {"channel": channel}, "id": 2});
                            ws.send(Message::Text(sub.to_string().into()))
                                .map_err(|e| e.to_string())?;
                        }
                        Some(2) => emit(Event::Line(
                            group.to_string(),
                            ChatLine::System("Joined Kick chat".into()),
                        )),
                        _ => {}
                    }
                    if let Some(push) = v.get("push") {
                        if push.get("disconnect").is_some() {
                            return Err("Kick ended the connection".into());
                        }
                        let data = push
                            .get("pub")
                            .and_then(|p| p.get("data"))
                            .cloned()
                            .unwrap_or(Value::Null);
                        let event = data.get("event").and_then(Value::as_str).unwrap_or("");
                        if is_chat_event(event) {
                            if let Some(line) = data.get("data").and_then(kick_message) {
                                emit(Event::Line(group.to_string(), line));
                            }
                        }
                    }
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

/// Kick's older Pusher chat (Moltorino ws/KickPusherManager.cpp).
fn pusher_session(
    group: &str,
    url: &str,
    chatroom_id: u64,
    stop: &AtomicBool,
    emit: &impl Fn(Event),
) -> Result<(), String> {
    let (mut ws, _) = tungstenite::connect(url).map_err(|e| e.to_string())?;
    set_short_timeout(&ws);
    let sub = serde_json::json!({"event": "pusher:subscribe", "data": {"auth": "", "channel": format!("chatrooms.{chatroom_id}.v2")}});
    ws.send(Message::Text(sub.to_string().into()))
        .map_err(|e| e.to_string())?;
    let mut last_ping = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        if last_ping.elapsed() > Duration::from_secs(20) {
            let _ = ws.send(Message::Text(r#"{"event":"pusher:ping","data":{}}"#.into()));
            last_ping = Instant::now();
        }
        match ws.read() {
            Ok(Message::Text(t)) => {
                let Ok(v) = serde_json::from_str::<Value>(&t) else {
                    continue;
                };
                let event = v.get("event").and_then(Value::as_str).unwrap_or("");
                if event == "pusher:ping" {
                    let _ = ws.send(Message::Text(r#"{"event":"pusher:pong","data":{}}"#.into()));
                } else if event == "pusher_internal:subscription_succeeded" {
                    emit(Event::Line(
                        group.to_string(),
                        ChatLine::System("Joined Kick chat".into()),
                    ));
                } else if is_chat_event(event) {
                    if let Some(line) = v.get("data").and_then(kick_message) {
                        emit(Event::Line(group.to_string(), line));
                    }
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
// ------------------------------------------------------------------ YouTube

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .http_status_as_error(false)
        .user_agent(UA)
        .build()
        .into()
}

fn between<'a>(hay: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let i = hay.find(start)? + start.len();
    let j = hay[i..].find(end)? + i;
    Some(&hay[i..j])
}

/// One chat action -> a line (text messages only; super chats show their text too).
fn yt_line(action: &Value) -> Option<ChatLine> {
    let item = action.get("addChatItemAction")?.get("item")?;
    let r = item
        .get("liveChatTextMessageRenderer")
        .or_else(|| item.get("liveChatPaidMessageRenderer"))?;
    let name = r
        .get("authorName")
        .and_then(|a| a.get("simpleText"))
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_string();
    let mut text = String::new();
    let mut segs = Vec::new();
    for run in r
        .get("message")
        .and_then(|m| m.get("runs"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(t) = run.get("text").and_then(Value::as_str) {
            text.push_str(t);
            segs.push(Seg::Text(t.to_string()));
        } else if let Some(emoji) = run.get("emoji") {
            let label = emoji
                .get("shortcuts")
                .and_then(Value::as_array)
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let custom = emoji
                .get("isCustomEmoji")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let url = emoji
                .get("image")
                .and_then(|i| i.get("thumbnails"))
                .and_then(Value::as_array)
                .and_then(|t| t.last())
                .and_then(|t| t.get("url"))
                .and_then(Value::as_str);
            match (custom, url) {
                (true, Some(u)) => segs.push(Seg::Emote(Emote {
                    name: label.clone(),
                    url: u.to_string(),
                    provider: Provider::YouTube,
                })),
                _ => {
                    // a normal emoji: its character
                    let ch = emoji
                        .get("emojiId")
                        .and_then(Value::as_str)
                        .unwrap_or(&label)
                        .to_string();
                    segs.push(Seg::Text(ch));
                }
            }
            text.push_str(&label);
        }
    }
    Some(msg(Platform::YouTube, name, None, text, segs))
}

fn yt_continuation(v: &Value) -> Option<(String, u64)> {
    let conts = v.get("continuations")?.as_array()?;
    for c in conts {
        for k in [
            "invalidationContinuationData",
            "timedContinuationData",
            "reloadContinuationData",
        ] {
            if let Some(d) = c.get(k) {
                let token = d.get("continuation").and_then(Value::as_str)?.to_string();
                let wait = d.get("timeoutMs").and_then(Value::as_u64).unwrap_or(5000);
                return Some((token, wait));
            }
        }
    }
    None
}

pub fn youtube(group: String, video_id: String, tx: Sender<Event>, ctx: egui::Context) -> Reader {
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    std::thread::spawn(move || {
        let emit = |e: Event| {
            let _ = tx.send(e);
            ctx.request_repaint();
        };
        while !stop2.load(Ordering::Relaxed) {
            if let Err(e) = youtube_session(&group, &video_id, &stop2, &emit) {
                emit(Event::Line(
                    group.clone(),
                    ChatLine::System(format!("YouTube chat: {e}")),
                ));
                for _ in 0..30 {
                    if stop2.load(Ordering::Relaxed) {
                        return;
                    }
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
        }
    });
    Reader { stop }
}

fn youtube_session(
    group: &str,
    video: &str,
    stop: &AtomicBool,
    emit: &impl Fn(Event),
) -> Result<(), String> {
    let agent = agent();
    let mut r = agent
        .get(&format!(
            "https://www.youtube.com/live_chat?is_popout=1&v={video}"
        ))
        .header("Cookie", "SOCS=CAI; CONSENT=YES+1")
        .header("Accept-Language", "en")
        .call()
        .map_err(|e| e.to_string())?;
    let page = r
        .body_mut()
        .with_config()
        .limit(16 * 1024 * 1024)
        .read_to_string()
        .map_err(|e| e.to_string())?;
    let key = between(&page, "\"INNERTUBE_API_KEY\":\"", "\"")
        .ok_or("no chat on this stream")?
        .to_string();
    let version = between(&page, "\"INNERTUBE_CONTEXT_CLIENT_VERSION\":\"", "\"")
        .unwrap_or("2.20240101.00.00")
        .to_string();
    let mut cont = between(&page, "\"continuation\":\"", "\"")
        .ok_or("chat is off for this stream")?
        .to_string();
    emit(Event::Line(
        group.to_string(),
        ChatLine::System("Joined YouTube chat (read only)".into()),
    ));
    let url = format!(
        "https://www.youtube.com/youtubei/v1/live_chat/get_live_chat?key={key}&prettyPrint=false"
    );
    while !stop.load(Ordering::Relaxed) {
        let body = serde_json::json!({
            "context": {"client": {"clientName": "WEB", "clientVersion": version}},
            "continuation": cont,
        });
        let mut r = agent
            .post(&url)
            .header("Content-Type", "application/json")
            .send(body.to_string())
            .map_err(|e| e.to_string())?;
        if r.status() != 200 {
            return Err(format!("YouTube answered {}", r.status()));
        }
        let v: Value = r
            .body_mut()
            .with_config()
            .limit(16 * 1024 * 1024)
            .read_json()
            .map_err(|e| e.to_string())?;
        let Some(lc) = v
            .get("continuationContents")
            .and_then(|c| c.get("liveChatContinuation"))
        else {
            return Err("the stream's chat ended".into());
        };
        for a in lc
            .get("actions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(line) = yt_line(a) {
                emit(Event::Line(group.to_string(), line));
            }
        }
        let (next, wait) = yt_continuation(lc).ok_or("the stream's chat ended")?;
        cont = next;
        let until = Instant::now() + Duration::from_millis(wait.clamp(1000, 10000));
        while Instant::now() < until {
            if stop.load(Ordering::Relaxed) {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kick_emote_tokens() {
        let s = kick_segments("hi [emote:37226:KEKW] there");
        assert_eq!(s.len(), 3);
        assert!(
            matches!(&s[1], Seg::Emote(e) if e.name == "KEKW" && e.url.ends_with("37226/fullsize"))
        );
    }

    /// Network: joins a Kick chatroom and a YouTube live chat and waits for lines.
    /// `cargo test -- --ignored reads_kick_and_youtube --nocapture`
    #[test]
    #[ignore]
    fn reads_kick_and_youtube() {
        let (tx, rx) = std::sync::mpsc::channel();
        let k = crate::platforms::kick_info("xqc").unwrap().unwrap();
        let _kr = kick(
            "g".into(),
            k.channel_id,
            k.chatroom_id,
            tx.clone(),
            egui::Context::default(),
        );
        let (_, video) = crate::platforms::youtube_live("@LofiGirl")
            .unwrap()
            .expect("lofi girl live");
        let _yr = youtube("g".into(), video, tx, egui::Context::default());
        let deadline = Instant::now() + Duration::from_secs(25);
        let (mut kick_ok, mut yt_ok) = (false, false);
        while Instant::now() < deadline && !(kick_ok && yt_ok) {
            if let Ok(Event::Line(_, l)) = rx.recv_timeout(Duration::from_millis(500)) {
                match &l {
                    ChatLine::System(t) => {
                        println!("{t}");
                        kick_ok |= t.starts_with("Joined Kick");
                        yt_ok |= t.starts_with("Joined YouTube");
                    }
                    ChatLine::Msg { platform, .. } => println!("{platform:?} message"),
                    ChatLine::AutoMod(_) => {}
                }
            }
        }
        assert!(kick_ok && yt_ok);
    }
}
