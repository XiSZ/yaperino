//! Kick, YouTube and TikTok: live checks, finding a streamer's other channels,
//! and Kick sending. Endpoints follow Moltorino (EEVA-inc/Moltorino):
//! - Kick: `https://kick.com/api/v2/channels/{slug}` (KickApi.cpp privateChannelInfo),
//!   sending `POST https://api.kick.com/public/v1/chat` (KickApi::sendMessage).
//! - YouTube: the channel's `/live` page; live when it links a video and says
//!   `"isLiveNow":true` (YouTubeApi.cpp).
//! - TikTok: `https://www.tiktok.com/api-live/user/room/` (status 2 = live).

use crate::model::{parse_iso8601, Stream};
use serde_json::Value;
use std::time::Duration;

const BROWSER_UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36";
pub const KICK_CHANNEL: &str = "https://kick.com/api/v2/channels/";
pub const KICK_SEND: &str = "https://api.kick.com/public/v1/chat";
pub const KICK_EMOTE: &str = "https://files.kick.com/emotes/";
pub const TIKTOK_ROOM: &str =
    "https://www.tiktok.com/api-live/user/room/?aid=1988&sourceType=54&uniqueId=";
pub const YOUTUBE: &str = "https://www.youtube.com/";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Platform {
    Twitch,
    Kick,
    YouTube,
    TikTok,
}

impl Platform {
    pub const ALL: [Platform; 4] = [
        Platform::Twitch,
        Platform::Kick,
        Platform::YouTube,
        Platform::TikTok,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Platform::Twitch => "Twitch",
            Platform::Kick => "Kick",
            Platform::YouTube => "YouTube",
            Platform::TikTok => "TikTok",
        }
    }
    /// Short tag drawn before names in the list and chat.
    pub fn tag(self) -> &'static str {
        match self {
            Platform::Twitch => "TW",
            Platform::Kick => "K",
            Platform::YouTube => "YT",
            Platform::TikTok => "TT",
        }
    }
    pub fn color(self) -> egui::Color32 {
        match self {
            Platform::Twitch => egui::Color32::from_rgb(0x91, 0x46, 0xff),
            Platform::Kick => egui::Color32::from_rgb(0x53, 0xfc, 0x18),
            Platform::YouTube => egui::Color32::from_rgb(0xff, 0x33, 0x33),
            Platform::TikTok => egui::Color32::from_rgb(0x25, 0xf4, 0xee),
        }
    }
}

impl Default for Platform {
    fn default() -> Self {
        Platform::Twitch
    }
}

/// A Twitch channel's channels on the other platforms.
#[derive(Clone, Default, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Links {
    /// The Twitch channel (for tabs opened from Kick, YouTube or TikTok).
    pub twitch: String,
    pub kick: String,
    /// `@handle` or a channel id `UC...`.
    pub youtube: String,
    pub tiktok: String,
    /// Found automatically (shown as a guess until the person confirms or edits it).
    pub guessed: bool,
    /// The lookup has run for this channel.
    pub searched: bool,
    /// Where the links came from, e.g. "Twitch profile", "Kick profile", "same name".
    pub found_from: String,
}

impl Links {
    pub fn get(&self, p: Platform) -> &str {
        match p {
            Platform::Kick => &self.kick,
            Platform::YouTube => &self.youtube,
            Platform::TikTok => &self.tiktok,
            Platform::Twitch => &self.twitch,
        }
    }
    pub fn set(&mut self, p: Platform, v: String) {
        match p {
            Platform::Kick => self.kick = v,
            Platform::YouTube => self.youtube = v,
            Platform::TikTok => self.tiktok = v,
            Platform::Twitch => self.twitch = v,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.kick.is_empty() && self.youtube.is_empty() && self.tiktok.is_empty()
    }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .http_status_as_error(false)
        .user_agent(BROWSER_UA)
        .build()
        .into()
}

/// Windows' own curl.exe (Schannel). Kick's bot protection refuses Rust's TLS
/// handshake with 403 but lets this through, as it does browsers.
fn system_curl() -> Option<std::path::PathBuf> {
    let p = std::path::PathBuf::from(std::env::var_os("WINDIR")?)
        .join("System32")
        .join("curl.exe");
    p.is_file().then_some(p)
}

/// One HTTP request through curl.exe. Headers go in on stdin (never on the command
/// line, where other programs could read a token). Returns (status, body).
fn curl(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
) -> Option<Result<(u16, String), String>> {
    curl_full(method, url, headers, body, false).map(|r| r.map(|(s, _, b)| (s, b)))
}

/// Like curl, also returning the response headers (for Set-Cookie).
pub fn curl_full(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
    with_headers: bool,
) -> Option<Result<(u16, String, String), String>> {
    curl_ua(method, url, headers, body, with_headers, BROWSER_UA)
}

/// curl as a given browser (a web sign-in's own user agent; empty = the default).
pub fn curl_as(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
    ua: &str,
) -> Result<(u16, String), String> {
    let ua = if ua.trim().is_empty() { BROWSER_UA } else { ua };
    curl_ua(method, url, headers, body, false, ua)
        .unwrap_or_else(|| Err("curl.exe not found".into()))
        .map(|(s, _, b)| (s, b))
}

fn curl_ua(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
    with_headers: bool,
    ua: &str,
) -> Option<Result<(u16, String, String), String>> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let exe = system_curl()?;
    let mut cmd = Command::new(exe);
    cmd.args([
        "-s",
        "-L",
        "--compressed",
        "--max-time",
        "15",
        "-X",
        method,
        "-A",
        ua,
        "-H",
        "@-",
        "-w",
        "\n%{http_code}",
    ]);
    if with_headers {
        cmd.arg("-i");
    }
    if let Some(b) = body {
        cmd.args(["--data-raw", b]);
    }
    cmd.arg(url)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // no console window
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return Some(Err(e.to_string())),
    };
    if let Some(mut stdin) = child.stdin.take() {
        let mut lines = String::from("Accept-Language: en\n");
        for (k, v) in headers {
            lines.push_str(&format!("{k}: {v}\n"));
        }
        let _ = stdin.write_all(lines.as_bytes());
    }
    let out = match child.wait_with_output() {
        Ok(o) => o,
        Err(e) => return Some(Err(e.to_string())),
    };
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let (rest, code) = text.rsplit_once('\n').unwrap_or(("", text.as_str()));
    let (head, body) = if with_headers {
        // with -L there may be several header blocks; the body follows the last one
        let mut head = String::new();
        let mut body = rest;
        while body.starts_with("HTTP/") {
            match body.split_once("\r\n\r\n") {
                Some((h, b)) => {
                    head.push_str(h);
                    head.push('\n');
                    body = b;
                }
                None => break,
            }
        }
        (head, body.to_string())
    } else {
        (String::new(), rest.to_string())
    };
    match code.trim().parse::<u16>() {
        Ok(0) | Err(_) => Some(Err("no answer (network?)".into())),
        Ok(c) => Some(Ok((c, head, body))),
    }
}

/// POST whose body holds secrets (client secret, codes): the body goes to curl.exe
/// on stdin (--data-binary @-), never on the command line.
pub fn curl_secret_body(
    url: &str,
    content_type: &str,
    body: &str,
) -> Result<(u16, String), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let exe = system_curl().ok_or("curl.exe not found")?;
    let mut cmd = Command::new(exe);
    cmd.args([
        "-s",
        "--max-time",
        "20",
        "-X",
        "POST",
        "-A",
        BROWSER_UA,
        "-H",
    ])
    .arg(format!("Content-Type: {content_type}"))
    .args(["--data-binary", "@-", "-w", "\n%{http_code}"])
    .arg(url)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(body.as_bytes());
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let (rest, code) = text.rsplit_once('\n').unwrap_or(("", text.as_str()));
    match code.trim().parse::<u16>() {
        Ok(0) | Err(_) => Err("no answer (network?)".into()),
        Ok(c) => Ok((c, rest.to_string())),
    }
}

fn get_text(url: &str, cookie: Option<&str>) -> Result<(u16, String), String> {
    let headers: Vec<(&str, &str)> = cookie.map(|c| vec![("Cookie", c)]).unwrap_or_default();
    if let Some(r) = curl("GET", url, &headers, None) {
        return r;
    }
    // no curl.exe: try directly (Kick may refuse this)
    let mut req = agent().get(url).header("Accept-Language", "en");
    if let Some(c) = cookie {
        req = req.header("Cookie", c);
    }
    let mut r = req.call().map_err(|e| e.to_string())?;
    let status = r.status().as_u16();
    let body = r
        .body_mut()
        .with_config()
        .limit(16 * 1024 * 1024)
        .read_to_string()
        .map_err(|e| e.to_string())?;
    Ok((status, body))
}
// ------------------------------------------------------------------ Kick

#[derive(Clone, Debug)]
pub struct KickInfo {
    pub slug: String,
    #[allow(dead_code)]
    pub username: String,
    /// Kick user id (the broadcaster id for sending).
    pub user_id: u64,
    /// Kick channel id (for the realtime chat connection).
    pub channel_id: u64,
    pub chatroom_id: u64,
    pub live: Option<Stream>,
    pub youtube: String,
    pub tiktok: String,
}

pub fn kick_info(slug: &str) -> Result<Option<KickInfo>, String> {
    let (status, body) = get_text(&format!("{KICK_CHANNEL}{}", slug.to_lowercase()), None)?;
    if status == 404 {
        return Ok(None);
    }
    if status != 200 {
        return Err(format!("Kick answered {status}"));
    }
    let v: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let user = v.get("user").cloned().unwrap_or(Value::Null);
    let s = |o: &Value, k: &str| o.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let username = s(&user, "username");
    let slug = s(&v, "slug");
    let live = v.get("livestream").filter(|l| l.is_object()).and_then(|l| {
        if l.get("is_live").and_then(Value::as_bool) == Some(false) {
            return None;
        }
        let started = l
            .get("start_time")
            .or_else(|| l.get("created_at"))
            .and_then(Value::as_str)
            .and_then(|t| parse_iso8601(&t.replace(' ', "T")))
            .unwrap_or_else(crate::model::now_unix);
        let game = l
            .get("categories")
            .and_then(Value::as_array)
            .and_then(|c| c.first())
            .map(|c| s(c, "name"))
            .unwrap_or_default();
        Some(Stream {
            login: format!("kick:{slug}"),
            display_name: if username.is_empty() {
                slug.clone()
            } else {
                username.clone()
            },
            title: s(l, "session_title"),
            game,
            viewers: l.get("viewer_count").and_then(Value::as_u64).unwrap_or(0),
            started,
            status_changed: started,
            list_index: usize::MAX,
            platform: Platform::Kick,
            group: String::new(),
        })
    });
    // socials on the Kick profile: "channel/UC..." or a handle, and the TikTok name
    let youtube = s(&user, "youtube");
    let youtube = youtube
        .trim_start_matches("channel/")
        .trim_start_matches('@')
        .trim_matches('/')
        .to_string();
    let youtube = if youtube.starts_with("UC") || youtube.is_empty() {
        youtube
    } else {
        format!("@{youtube}")
    };
    Ok(Some(KickInfo {
        slug,
        username,
        user_id: v.get("user_id").and_then(Value::as_u64).unwrap_or(0),
        channel_id: v.get("id").and_then(Value::as_u64).unwrap_or(0),
        chatroom_id: v
            .get("chatroom")
            .and_then(|c| c.get("id"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        live,
        youtube,
        tiktok: s(&user, "tiktok")
            .trim_start_matches('@')
            .trim_matches('/')
            .to_string(),
    }))
}

/// Sends a Kick chat message (Kick's public API, as Moltorino does), with this app's own Kick sign-in.
pub fn kick_send(token: &str, broadcaster_user_id: u64, text: &str) -> Result<(), String> {
    let body = serde_json::json!({ "broadcaster_user_id": broadcaster_user_id, "content": text, "type": "user" }).to_string();
    let auth = format!("Bearer {token}");
    let headers = [
        ("Authorization", auth.as_str()),
        ("Content-Type", "application/json"),
    ];
    let (status, answer) = match curl("POST", KICK_SEND, &headers, Some(&body)) {
        Some(r) => r?,
        None => {
            let mut r = agent()
                .post(KICK_SEND)
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
                .send(body.clone())
                .map_err(|e| e.to_string())?;
            let s = r.status().as_u16();
            (s, r.body_mut().read_to_string().unwrap_or_default())
        }
    };
    let v: Value = serde_json::from_str(&answer).unwrap_or(Value::Null);
    let sent = v
        .get("data")
        .and_then(|d| d.get("is_sent"))
        .and_then(Value::as_bool)
        .unwrap_or(status == 200);
    if status == 200 && sent {
        Ok(())
    } else if status == 401 {
        Err("Kick ended the sign-in: sign in again (Settings > Accounts)".into())
    } else {
        Err(v
            .get("message")
            .and_then(Value::as_str)
            .map(String::from)
            .unwrap_or_else(|| format!("Kick answered {status}")))
    }
}
/// How to connect to a Kick chatroom right now (Moltorino KickLiveUpdates.cpp /
/// KickCentrifugoManager.cpp): Kick answers with a Centrifugo or Pusher server.
pub enum KickRealtime {
    /// Centrifugo websocket url and a guest token.
    Centrifugo(String, String),
    Pusher(String),
}

pub const KICK_REALTIME: &str = "https://web.kick.com/api/v1/realtime/";

pub fn kick_realtime(channel_id: u64) -> Result<KickRealtime, String> {
    let body = r#"{"client":{"id":"js","type":"web"},"capabilities":{"accepted_providers":[{"provider":"pusher"},{"provider":"centrifugo"}]}}"#;
    let url = format!("{KICK_REALTIME}channels/{channel_id}/chat/connection");
    let (status, answer) = curl(
        "POST",
        &url,
        &[("Content-Type", "application/json")],
        Some(body),
    )
    .unwrap_or_else(|| Err("curl.exe not found".into()))?;
    if status != 200 {
        return Err(format!("Kick answered {status}"));
    }
    let v: Value = serde_json::from_str(&answer).map_err(|e| e.to_string())?;
    for c in v
        .get("data")
        .and_then(|d| d.get("connections"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let creds = c.get("credentials").cloned().unwrap_or(Value::Null);
        match c.get("provider").and_then(Value::as_str) {
            Some("centrifugo") => {
                let ws = creds.get("url").and_then(Value::as_str).unwrap_or("");
                if !ws.starts_with("wss://") {
                    continue;
                }
                let (s, a) = curl(
                    "POST",
                    &format!("{KICK_REALTIME}auth/connection"),
                    &[("Content-Type", "application/json")],
                    Some(r#"{"client_id":"js"}"#),
                )
                .unwrap_or_else(|| Err("curl.exe not found".into()))?;
                if s != 200 {
                    return Err(format!("Kick chat token: {s}"));
                }
                let t: Value = serde_json::from_str(&a).map_err(|e| e.to_string())?;
                let token = t
                    .get("data")
                    .and_then(|d| d.get("token"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if token.is_empty() {
                    return Err("Kick gave no chat token".into());
                }
                return Ok(KickRealtime::Centrifugo(ws.to_string(), token.to_string()));
            }
            Some("pusher") => {
                let key = creds
                    .get("app_key")
                    .and_then(Value::as_str)
                    .unwrap_or("32cbd69e4b950bf97679");
                let cluster = creds
                    .get("cluster")
                    .and_then(Value::as_str)
                    .unwrap_or("us2");
                return Ok(KickRealtime::Pusher(format!(
                    "wss://ws-{cluster}.pusher.com/app/{key}?protocol=7&client=js&version=8.4.0&flash=false"
                )));
            }
            _ => {}
        }
    }
    Err("Kick offered no chat connection".into())
}

// ------------------------------------------------------------------ YouTube

/// The page of a channel: `@handle` or `UC...` id.
pub fn youtube_url(channel: &str) -> String {
    let c = channel.trim().trim_start_matches(YOUTUBE);
    if c.starts_with("UC") {
        format!("{YOUTUBE}channel/{c}")
    } else {
        format!("{YOUTUBE}@{}", c.trim_start_matches('@'))
    }
}

pub fn between<'a>(hay: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let i = hay.find(start)? + start.len();
    let j = hay[i..].find(end)? + i;
    Some(&hay[i..j])
}

fn json_unescape(s: &str) -> String {
    serde_json::from_str::<String>(&format!("\"{s}\"")).unwrap_or_else(|_| s.to_string())
}

/// (stream, video id) when the channel is live.
pub fn youtube_live(channel: &str) -> Result<Option<(Stream, String)>, String> {
    let (status, page) = get_text(
        &format!("{}/live", youtube_url(channel)),
        Some("SOCS=CAI; CONSENT=YES+1"),
    )?;
    if status == 404 {
        return Ok(None);
    }
    if status != 200 {
        return Err(format!("YouTube answered {status}"));
    }
    let video = between(
        &page,
        "rel=\"canonical\" href=\"https://www.youtube.com/watch?v=",
        "\"",
    )
    .unwrap_or("");
    if video.is_empty() || !page.contains("\"isLiveNow\":true") {
        return Ok(None);
    }
    let title = between(&page, "<meta name=\"title\" content=\"", "\"")
        .map(html_unescape)
        .unwrap_or_default();
    let name = between(&page, "\"ownerChannelName\":\"", "\"")
        .map(json_unescape)
        .unwrap_or_else(|| channel.to_string());
    let viewers = between(&page, "\"originalViewCount\":\"", "\"")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let started = between(&page, "\"startTimestamp\":\"", "\"")
        .and_then(parse_iso8601)
        .unwrap_or_else(crate::model::now_unix);
    let game = between(&page, "\"category\":\"", "\"")
        .map(json_unescape)
        .unwrap_or_default();
    let key = channel.trim_start_matches('@').to_lowercase();
    Ok(Some((
        Stream {
            login: format!("youtube:{key}"),
            display_name: name,
            title,
            game,
            viewers,
            started,
            status_changed: started,
            list_index: usize::MAX,
            platform: Platform::YouTube,
            group: String::new(),
        },
        video.to_string(),
    )))
}

fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

// ------------------------------------------------------------------ TikTok

pub fn tiktok_live(user: &str) -> Result<Option<Stream>, String> {
    let user = user.trim().trim_start_matches('@');
    let (status, body) = get_text(&format!("{TIKTOK_ROOM}{user}"), None)?;
    if status != 200 {
        return Err(format!("TikTok answered {status}"));
    }
    let v: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let data = v.get("data").ok_or("TikTok: no such user")?;
    let u = data.get("user").cloned().unwrap_or(Value::Null);
    // status 2 = live (4 = ended)
    if u.get("status").and_then(Value::as_i64) != Some(2) {
        return Ok(None);
    }
    let room = data.get("liveRoom").cloned().unwrap_or(Value::Null);
    let started = room
        .get("startTime")
        .and_then(Value::as_i64)
        .unwrap_or_else(crate::model::now_unix);
    let s = |o: &Value, k: &str| o.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let nick = s(&u, "nickname");
    Ok(Some(Stream {
        login: format!("tiktok:{}", user.to_lowercase()),
        display_name: if nick.is_empty() {
            user.to_string()
        } else {
            nick
        },
        title: s(&room, "title"),
        game: String::new(),
        viewers: room
            .get("liveRoomStats")
            .and_then(|x| x.get("userCount"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        started,
        status_changed: started,
        list_index: usize::MAX,
        platform: Platform::TikTok,
        group: String::new(),
    }))
}

/// Does a TikTok user exist (for guessing links)?
pub fn tiktok_exists(user: &str) -> bool {
    get_text(
        &format!("{TIKTOK_ROOM}{}", user.trim_start_matches('@')),
        None,
    )
    .ok()
    .and_then(|(s, b)| (s == 200).then_some(b))
    .and_then(|b| serde_json::from_str::<Value>(&b).ok())
    .and_then(|v| {
        v.get("data")
            .and_then(|d| d.get("user"))
            .and_then(|u| u.get("uniqueId"))
            .cloned()
    })
    .is_some()
}

/// Does a YouTube @handle exist?
pub fn youtube_exists(handle: &str) -> bool {
    get_text(&youtube_url(handle), Some("SOCS=CAI; CONSENT=YES+1"))
        .map(|(s, _)| s == 200)
        .unwrap_or(false)
}

// ------------------------------------------------------------------ finding a streamer everywhere

/// Twitch's public web client id: the same one twitch.tv's own pages and Streamlink use
/// for public, signed-out reads. No token is ever sent with it.
const TWITCH_WEB_CLIENT_ID: &str = "kimne78kx3ncx6brgo4mv6wki5h1ko";
pub const TWITCH_GQL: &str = "https://gql.twitch.tv/gql";

/// The links on a Twitch channel's "About" page (Twitch's public web data).
pub fn twitch_socials(login: &str) -> Result<Vec<String>, String> {
    let q = serde_json::json!({
        "query": format!("query {{ user(login: \"{}\") {{ channel {{ socialMedias {{ url }} }} }} }}", login.to_lowercase())
    });
    let mut r = agent()
        .post(TWITCH_GQL)
        .header("Client-Id", TWITCH_WEB_CLIENT_ID)
        .header("Content-Type", "application/json")
        .send(q.to_string())
        .map_err(|e| e.to_string())?;
    if r.status() != 200 {
        return Err(format!("Twitch answered {}", r.status()));
    }
    let v: Value = r.body_mut().read_json().map_err(|e| e.to_string())?;
    let list = v
        .pointer("/data/user/channel/socialMedias")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|s| s.get("url").and_then(Value::as_str).map(String::from))
                .collect()
        })
        .unwrap_or_default();
    Ok(list)
}

/// Does a Twitch channel exist (IVR, as the live check uses)?
pub fn twitch_exists(login: &str) -> bool {
    get_text(
        &format!("{}{}", crate::fetch::IVR_URL, login.to_lowercase()),
        None,
    )
    .ok()
    .and_then(|(s, b)| (s == 200).then_some(b))
    .and_then(|b| serde_json::from_str::<Value>(&b).ok())
    .and_then(|v| v.as_array().map(|a| !a.is_empty()))
    .unwrap_or(false)
}

/// A profile link -> (platform, channel name): youtube.com/channel/UC..., /@handle,
/// /c/name, /user/name; tiktok.com/@name; kick.com/name; twitch.tv/name.
pub fn parse_social(url: &str) -> Option<(Platform, String)> {
    let u = url.trim();
    let u = u.split("://").nth(1).unwrap_or(u);
    let u = u.trim_start_matches("www.").trim_start_matches("m.");
    let (host, path) = u.split_once('/').unwrap_or((u, ""));
    let path = path
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .trim_matches('/');
    let first = path.split('/').next().unwrap_or("");
    let second = path.split('/').nth(1).unwrap_or("");
    let clean = |s: &str| s.trim_start_matches('@').to_string();
    match host {
        "youtube.com" | "youtu.be" => {
            if first == "channel" && second.starts_with("UC") {
                Some((Platform::YouTube, second.to_string()))
            } else if first.starts_with('@') {
                Some((Platform::YouTube, first.to_string()))
            } else if (first == "c" || first == "user") && !second.is_empty() {
                Some((Platform::YouTube, format!("@{second}")))
            } else {
                None
            }
        }
        "tiktok.com" if first.starts_with('@') => Some((Platform::TikTok, clean(first))),
        "kick.com" if !first.is_empty() => Some((Platform::Kick, first.to_lowercase())),
        "twitch.tv" if !first.is_empty() => Some((Platform::Twitch, first.to_lowercase())),
        _ => None,
    }
}

/// Finds a streamer on every platform, starting from the channel the person entered:
/// 1. Twitch profile links (confirmed),
/// 2. Kick profile links,
/// 3. the same name on the other platforms (a guess, shown as such).
pub fn discover_from(platform: Platform, name: &str) -> Links {
    let mut links = Links {
        searched: true,
        ..Default::default()
    };
    links.set(platform, name.to_string());
    let mut sources: Vec<&str> = Vec::new();
    let mut guessed = false;

    // Twitch: the entered channel, or the same name when it links back
    let bare = name.trim_start_matches('@').to_lowercase();
    if platform != Platform::Twitch
        && !bare.is_empty()
        && !bare.starts_with("uc")
        && twitch_exists(&bare)
    {
        let socials = twitch_socials(&bare).unwrap_or_default();
        let links_back = socials
            .iter()
            .filter_map(|s| parse_social(s))
            .any(|(p, n)| p == platform && n.trim_start_matches('@').eq_ignore_ascii_case(&bare));
        links.twitch = bare.clone();
        if links_back {
            sources.push("Twitch profile links back");
        } else {
            guessed = true;
        }
    }
    if !links.twitch.is_empty() {
        if let Ok(socials) = twitch_socials(&links.twitch) {
            let mut any = false;
            for (p, n) in socials.iter().filter_map(|s| parse_social(s)) {
                if p != Platform::Twitch && links.get(p).is_empty() {
                    links.set(p, n);
                    any = true;
                }
            }
            if any {
                sources.push("Twitch profile");
            }
        }
    }

    // Kick: the linked or same-name Kick profile lists YouTube and TikTok
    let kick_name = if !links.kick.is_empty() {
        links.kick.clone()
    } else if !links.twitch.is_empty() {
        links.twitch.clone()
    } else {
        bare.clone()
    };
    if !kick_name.is_empty() {
        if let Ok(Some(k)) = kick_info(&kick_name) {
            if links.kick.is_empty() {
                links.kick = k.slug;
                guessed |= platform != Platform::Kick;
            }
            let mut any = false;
            if links.youtube.is_empty() && !k.youtube.is_empty() {
                links.youtube = k.youtube;
                any = true;
            }
            if links.tiktok.is_empty() && !k.tiktok.is_empty() {
                links.tiktok = k.tiktok;
                any = true;
            }
            if any {
                sources.push("Kick profile");
            }
        }
    }

    // the same name elsewhere (a guess)
    let base = if links.twitch.is_empty() {
        bare.clone()
    } else {
        links.twitch.clone()
    };
    if !base.is_empty() && !base.starts_with("uc") {
        if links.youtube.is_empty() && youtube_exists(&format!("@{base}")) {
            links.youtube = format!("@{base}");
            guessed = true;
        }
        if links.tiktok.is_empty() && tiktok_exists(&base) {
            links.tiktok = base.clone();
            guessed = true;
        }
    }
    if guessed {
        sources.push("same name (please check)");
    }
    links.guessed = guessed;
    links.found_from = sources.join(", ");
    links
}

/// Finds a Twitch streamer's other channels.
pub fn discover(twitch_login: &str) -> Links {
    discover_from(Platform::Twitch, twitch_login)
}
/// Live streams on the other platforms for linked channels; `group` = the Twitch login.
pub struct OtherLive {
    pub streams: Vec<Stream>,
    /// Kick: group -> (broadcaster user id, channel id, chatroom id)
    pub kick_ids: Vec<(String, u64, u64, u64)>,
    /// YouTube: group -> live video id
    pub youtube_videos: Vec<(String, String)>,
    /// Checks that failed this round (kept for the Info window and tests).
    #[allow(dead_code)]
    pub errors: Vec<String>,
}

/// What one Kick / YouTube / TikTok check found.
#[derive(Clone, Default)]
struct Checked {
    stream: Option<Stream>,
    kick_ids: Option<(u64, u64, u64)>,
    video: Option<String>,
    error: Option<String>,
}

fn check_one(p: Platform, name: &str) -> Checked {
    let mut c = Checked::default();
    match p {
        Platform::Kick => match kick_info(name) {
            Ok(Some(k)) => {
                c.kick_ids = Some((k.user_id, k.channel_id, k.chatroom_id));
                c.stream = k.live;
            }
            Ok(None) => {}
            Err(e) => c.error = Some(format!("Kick {name}: {e}")),
        },
        Platform::YouTube => match youtube_live(name) {
            Ok(Some((s, video))) => {
                c.stream = Some(s);
                c.video = Some(video);
            }
            Ok(None) => {}
            Err(e) => c.error = Some(format!("YouTube {name}: {e}")),
        },
        Platform::TikTok => match tiktok_live(name) {
            Ok(Some(s)) => c.stream = Some(s),
            Ok(None) => {}
            Err(e) => c.error = Some(format!("TikTok {name}: {e}")),
        },
        Platform::Twitch => {}
    }
    c
}

/// How often a channel that has no open tab is checked again. Kick answers small JSON and
/// is checked every round; a YouTube check loads a whole page and TikTok limits requests,
/// so channels found by name there are spread over several rounds.
pub fn recheck_after(p: Platform) -> Duration {
    match p {
        Platform::YouTube | Platform::TikTok => Duration::from_secs(300),
        _ => Duration::ZERO,
    }
}

/// Checks at once (several channels in parallel, like a browser loading a page).
pub const CHECK_THREADS: usize = 6;

/// Live checks for every known channel on Kick, YouTube and TikTok, keeping the last
/// answer of channels not due this round, so they don't look offline in between.
#[derive(Default)]
pub struct LinkChecker {
    cache: std::collections::HashMap<(Platform, String), (std::time::Instant, Checked)>,
}

impl LinkChecker {
    /// `links`: group (tab key or Twitch login) -> its channels; `open`: groups with an open tab
    /// (always checked now).
    pub fn check(
        &mut self,
        links: &[(String, Links)],
        open: &std::collections::HashSet<String>,
    ) -> OtherLive {
        use std::collections::HashMap;
        let mut owners: Vec<((Platform, String), String)> = Vec::new();
        for (group, l) in links {
            for p in [Platform::Kick, Platform::YouTube, Platform::TikTok] {
                let n = l.get(p).trim().to_string();
                if !n.is_empty() {
                    // YouTube channel ids (UC...) are case-sensitive
                    let n = if p == Platform::YouTube {
                        n
                    } else {
                        n.to_lowercase()
                    };
                    owners.push(((p, n), group.clone()));
                }
            }
        }
        // open tabs first, so a channel shared by two groups shows under the open one
        owners.sort_by_key(|(_, g)| !open.contains(g));
        let now = std::time::Instant::now();
        let mut due: Vec<(Platform, String)> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for (key, group) in &owners {
            if !seen.insert(key.clone()) {
                continue;
            }
            let fresh = self
                .cache
                .get(key)
                .is_some_and(|(at, _)| now.duration_since(*at) < recheck_after(key.0));
            if open.contains(group) || !fresh {
                due.push(key.clone());
            }
        }
        let queue = std::sync::Mutex::new(due);
        let done: std::sync::Mutex<Vec<((Platform, String), Checked)>> =
            std::sync::Mutex::new(Vec::new());
        std::thread::scope(|s| {
            for _ in 0..CHECK_THREADS {
                s.spawn(|| loop {
                    let next = queue.lock().ok().and_then(|mut q| q.pop());
                    let Some((p, n)) = next else { break };
                    let c = check_one(p, &n);
                    if let Ok(mut d) = done.lock() {
                        d.push(((p, n), c));
                    }
                });
            }
        });
        let mut errors = Vec::new();
        for (key, c) in done.into_inner().unwrap_or_default() {
            if let Some(e) = &c.error {
                errors.push(e.clone());
            }
            self.cache.insert(key, (now, c));
        }
        let mut out = OtherLive {
            streams: Vec::new(),
            kick_ids: Vec::new(),
            youtube_videos: Vec::new(),
            errors,
        };
        let mut shown: HashMap<String, ()> = HashMap::new();
        for (key, group) in &owners {
            let Some((_, c)) = self.cache.get(key) else {
                continue;
            };
            if let Some(ids) = c.kick_ids {
                out.kick_ids.push((group.clone(), ids.0, ids.1, ids.2));
            }
            if let Some(v) = &c.video {
                out.youtube_videos.push((group.clone(), v.clone()));
            }
            if let Some(s) = &c.stream {
                if shown.insert(s.login.clone(), ()).is_none() {
                    let mut s = s.clone();
                    s.group = group.clone();
                    out.streams.push(s);
                }
            }
        }
        // forget channels no longer known
        let known: std::collections::HashSet<&(Platform, String)> =
            owners.iter().map(|(k, _)| k).collect();
        self.cache.retain(|k, _| known.contains(k));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Network: `cargo test -- --ignored platform_checks --nocapture`
    #[test]
    #[ignore]
    fn platform_checks() {
        let k = kick_info("xqc").expect("kick").expect("exists");
        println!(
            "kick: user {} chatroom {} live {} yt '{}' tt '{}'",
            k.user_id,
            k.chatroom_id,
            k.live.is_some(),
            k.youtube,
            k.tiktok
        );
        let y = youtube_live("@LofiGirl").expect("yt");
        println!(
            "youtube live: {:?}",
            y.as_ref()
                .map(|(s, v)| (s.viewers, s.title.clone(), v.clone()))
        );
        let t = tiktok_live("tiktok").expect("tt");
        println!("tiktok live: {}", t.is_some());
        assert!(k.chatroom_id > 0);
    }

    #[test]
    fn parses_profile_links() {
        assert_eq!(
            parse_social("https://www.youtube.com/channel/UCmDTrq0LNgPodDOFZiSbsww"),
            Some((Platform::YouTube, "UCmDTrq0LNgPodDOFZiSbsww".into()))
        );
        assert_eq!(
            parse_social("https://youtube.com/@LofiGirl/videos"),
            Some((Platform::YouTube, "@LofiGirl".into()))
        );
        assert_eq!(
            parse_social("https://www.tiktok.com/@someone?lang=en"),
            Some((Platform::TikTok, "someone".into()))
        );
        assert_eq!(
            parse_social("kick.com/SomeOne"),
            Some((Platform::Kick, "someone".into()))
        );
        assert_eq!(parse_social("https://twitter.com/x"), None);
    }

    /// Network: `cargo test -- --ignored finds_streamer_everywhere --nocapture`
    #[test]
    #[ignore]
    fn finds_streamer_everywhere() {
        let from_twitch = discover_from(Platform::Twitch, "xqc");
        println!("from Twitch: {from_twitch:?}");
        let from_kick = discover_from(Platform::Kick, "xqc");
        println!("from Kick: {from_kick:?}");
        assert!(!from_twitch.youtube.is_empty());
        assert_eq!(from_kick.twitch, "xqc");
    }
}
