//! YouTube sign-in and sending, with the person's own Google app (Desktop client).
//!
//! Google's installed-app sign-in: the browser opens Google's page, Google sends
//! the person back to `http://127.0.0.1:{port}` where this app listens once; the
//! code is exchanged for tokens with PKCE (S256) and a `state` check. Scope
//! `youtube.force-ssl` (needed to post chat messages). This app keeps its own
//! refresh token and may renew its own access token; Moltorino's Google sign-in is
//! never touched.
//!
//! Sending (YouTube Data API, as Moltorino's YouTubeChannel.cpp does):
//! `videos?part=liveStreamingDetails&id=VIDEO` -> activeLiveChatId, then
//! `POST liveChat/messages?part=snippet`.

use serde_json::Value;
use sha2::Digest;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

pub const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
pub const SCOPE: &str = "https://www.googleapis.com/auth/youtube.force-ssl";
pub const API: &str = "https://www.googleapis.com/youtube/v3/";

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct YouTubeSaved {
    pub client_id: String,
    pub client_secret: String,
    pub refresh_token: String,
    /// The signed-in channel's name (shown in Settings).
    pub channel_name: String,
    /// Signed in on youtube.com itself in the sign-in window (no Google app needed).
    pub session: crate::weblogin::WebSession,
}

impl YouTubeSaved {
    pub fn signed_in(&self) -> bool {
        !self.refresh_token.is_empty() || self.session.get("SAPISID").is_some()
    }
}

// ---------------------------------------------------------------- youtube.com sign-in (no app)
//
// As youtube.com's own chat box sends: the live chat page (with the person's cookies)
// gives the send button's `params`, then `youtubei/v1/live_chat/send_message` with the
// SAPISIDHASH authorization the site computes from the SAPISID cookie.

const ORIGIN: &str = "https://www.youtube.com";
const WEB_VERSION: &str = "2.20250101.00.00";

fn sha1_hex(s: &str) -> String {
    use sha1::Digest;
    sha1::Sha1::digest(s.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// `SAPISIDHASH <time>_<sha1(time SAPISID origin)>` (and the 1P/3P forms when there).
pub fn sapisid_hash(s: &crate::weblogin::WebSession, now: i64) -> Option<String> {
    let parts: Vec<String> = [
        ("SAPISID", "SAPISIDHASH"),
        ("__Secure-1PAPISID", "SAPISID1PHASH"),
        ("__Secure-3PAPISID", "SAPISID3PHASH"),
    ]
    .iter()
    .filter_map(|(c, label)| {
        s.get(c)
            .map(|v| format!("{label} {now}_{}", sha1_hex(&format!("{now} {v} {ORIGIN}"))))
    })
    .collect();
    (!parts.is_empty()).then(|| parts.join(" "))
}

fn web_post(
    s: &crate::weblogin::WebSession,
    path: &str,
    version: &str,
    body: &Value,
) -> Result<(u16, String), String> {
    let auth = sapisid_hash(s, crate::model::now_unix()).ok_or("Not signed in to YouTube")?;
    let cookie = s.header();
    let headers = [
        ("Cookie", cookie.as_str()),
        ("Authorization", auth.as_str()),
        ("X-Origin", ORIGIN),
        ("Origin", ORIGIN),
        ("X-Goog-AuthUser", "0"),
        ("X-Youtube-Client-Name", "1"),
        ("X-Youtube-Client-Version", version),
        ("Content-Type", "application/json"),
    ];
    crate::platforms::curl_as(
        "POST",
        &format!("{ORIGIN}/youtubei/v1/{path}?prettyPrint=false"),
        &headers,
        Some(&body.to_string()),
        &s.user_agent,
    )
}

/// The signed-in channel's name (only reads).
pub fn web_channel_name(s: &crate::weblogin::WebSession) -> Result<String, String> {
    let body = serde_json::json!({"context": {"client": {"clientName": "WEB", "clientVersion": WEB_VERSION, "hl": "en"}}});
    let (status, answer) = web_post(s, "account/account_menu", WEB_VERSION, &body)?;
    if status != 200 {
        return Err(format!("YouTube answered {status}"));
    }
    Ok(
        crate::platforms::between(&answer, "\"accountName\":{\"simpleText\":\"", "\"")
            .unwrap_or("")
            .to_string(),
    )
}

/// Sends to a live video's chat with the youtube.com sign-in.
pub fn web_send(s: &crate::weblogin::WebSession, video_id: &str, text: &str) -> Result<(), String> {
    let cookie = s.header();
    let (status, page) = crate::platforms::curl_as(
        "GET",
        &format!("{ORIGIN}/live_chat?is_popout=1&v={video_id}"),
        &[("Cookie", cookie.as_str())],
        None,
        &s.user_agent,
    )?;
    if status != 200 {
        return Err(format!("YouTube answered {status}"));
    }
    let at = page.find("\"sendLiveChatMessageEndpoint\"").ok_or(
        "YouTube shows no chat box here (signed out, or chat is for members or subscribers only)",
    )?;
    let params = crate::platforms::between(&page[at..], "\"params\":\"", "\"")
        .ok_or("YouTube's chat box has changed")?
        .to_string();
    let version = crate::platforms::between(&page, "\"INNERTUBE_CONTEXT_CLIENT_VERSION\":\"", "\"")
        .unwrap_or(WEB_VERSION)
        .to_string();
    let body = serde_json::json!({
        "context": {"client": {"clientName": "WEB", "clientVersion": version, "hl": "en"}},
        "params": params,
        "richMessage": {"textSegments": [{"text": text}]}
    });
    let (status, answer) = web_post(s, "live_chat/send_message", &version, &body)?;
    if status == 200 && !answer.contains("\"errorMessage\"") {
        return Ok(());
    }
    if status == 401 || status == 403 {
        return Err("YouTube ended the sign-in: sign in again (Settings > Accounts)".into());
    }
    let v: Value = serde_json::from_str(&answer).unwrap_or(Value::Null);
    Err(v
        .pointer("/error/message")
        .and_then(Value::as_str)
        .map(String::from)
        .or_else(|| {
            crate::platforms::between(&answer, "\"errorMessage\"", "}")
                .and_then(|e| crate::platforms::between(e, "\"text\":\"", "\""))
                .map(String::from)
        })
        .unwrap_or_else(|| format!("YouTube answered {status}")))
}

impl std::fmt::Debug for YouTubeSaved {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("YouTubeSaved")
            .field("channel_name", &self.channel_name)
            .finish()
    }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .http_status_as_error(false)
        .build()
        .into()
}

fn b64url(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        let count = chunk.len() + 1;
        for i in 0..count {
            out.push(A[((n >> (18 - 6 * i)) & 63) as usize] as char);
        }
    }
    out
}

fn random_string(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    let _ = getrandom::fill(&mut buf);
    b64url(&buf)
}

fn url_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// A random state value for a sign-in.
pub fn random_state() -> String {
    random_string(16)
}

/// PKCE: (verifier, S256 challenge).
pub fn pkce() -> (String, String) {
    let verifier = random_string(48);
    let challenge = b64url(&sha2::Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

pub fn auth_url(client_id: &str, redirect: &str, state: &str, challenge: &str) -> String {
    format!(
        "{AUTH_URL}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}&code_challenge={}&code_challenge_method=S256&access_type=offline&prompt=consent",
        url_encode(client_id),
        url_encode(redirect),
        url_encode(SCOPE),
        url_encode(state),
        url_encode(challenge)
    )
}

/// Waits (up to 5 minutes) for Google to send the browser back with `?code=...&state=...`.
pub fn wait_for_code(listener: TcpListener, state: &str) -> Result<String, String> {
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(300);
    while Instant::now() < deadline {
        match listener.accept() {
            Ok((mut sock, _)) => {
                let _ = sock.set_nonblocking(false);
                let _ = sock.set_read_timeout(Some(Duration::from_secs(5)));
                let mut buf = [0u8; 8192];
                let n = sock.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = req.split_whitespace().nth(1).unwrap_or("");
                let query = path.split_once('?').map(|x| x.1).unwrap_or("");
                let param = |k: &str| {
                    query.split('&').find_map(|kv| {
                        kv.split_once('=')
                            .filter(|(a, _)| *a == k)
                            .map(|(_, v)| url_decode(v))
                    })
                };
                let (code, got_state, error) = (param("code"), param("state"), param("error"));
                let ok = code.is_some() && got_state.as_deref() == Some(state);
                let page = if ok {
                    "Signed in. You can close this tab and go back to Live Streams."
                } else {
                    "Sign-in did not finish. You can close this tab and try again."
                };
                let _ = write!(
                    sock,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n<html><body style=\"font-family:sans-serif\"><h3>{page}</h3></body></html>"
                );
                if let Some(e) = error {
                    return Err(format!("Google: {e}"));
                }
                if ok {
                    return Ok(code.unwrap_or_default());
                }
                if code.is_some() {
                    return Err("The sign-in answer did not match (state)".into());
                }
                // something else (favicon...): keep waiting
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100))
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("Sign-in timed out".into())
}

fn url_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                    out.push(v);
                    i += 3;
                    continue;
                }
                out.push(b'%');
            }
            b'+' => out.push(b' '),
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn token_request(form: &[(&str, &str)]) -> Result<Value, String> {
    let mut r = agent()
        .post(TOKEN_URL)
        .send_form(form.iter().copied())
        .map_err(|e| e.to_string())?;
    let v: Value = r.body_mut().read_json().map_err(|e| e.to_string())?;
    if v.get("access_token").is_some() {
        Ok(v)
    } else {
        Err(v
            .get("error_description")
            .or_else(|| v.get("error"))
            .and_then(Value::as_str)
            .unwrap_or("Google refused the sign-in")
            .to_string())
    }
}

/// The whole sign-in: opens the browser, waits, exchanges the code. Returns the
/// saved sign-in (with the channel name).
pub fn sign_in(
    client_id: &str,
    client_secret: &str,
    open_browser: impl Fn(&str),
) -> Result<YouTubeSaved, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let redirect = format!("http://127.0.0.1:{port}");
    let state = random_string(16);
    let (verifier, challenge) = pkce();
    open_browser(&auth_url(client_id, &redirect, &state, &challenge));
    let code = wait_for_code(listener, &state)?;
    let v = token_request(&[
        ("code", &code),
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("redirect_uri", &redirect),
        ("grant_type", "authorization_code"),
        ("code_verifier", &verifier),
    ])?;
    let refresh = v
        .get("refresh_token")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if refresh.is_empty() {
        return Err("Google gave no lasting sign-in (refresh token); try again".into());
    }
    let access = v
        .get("access_token")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let channel_name = my_channel(&access).unwrap_or_default();
    Ok(YouTubeSaved {
        client_id: client_id.into(),
        client_secret: client_secret.into(),
        refresh_token: refresh,
        channel_name,
        ..Default::default()
    })
}

/// A fresh access token from this app's own refresh token.
pub fn access_token(saved: &YouTubeSaved) -> Result<String, String> {
    let v = token_request(&[
        ("client_id", &saved.client_id),
        ("client_secret", &saved.client_secret),
        ("refresh_token", &saved.refresh_token),
        ("grant_type", "refresh_token"),
    ])?;
    Ok(v.get("access_token")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string())
}

fn api_get(path: &str, token: &str) -> Result<Value, String> {
    let mut r = agent()
        .get(&format!("{API}{path}"))
        .header("Authorization", &format!("Bearer {token}"))
        .call()
        .map_err(|e| e.to_string())?;
    let status = r.status().as_u16();
    let v: Value = r.body_mut().read_json().map_err(|e| e.to_string())?;
    if status == 200 {
        Ok(v)
    } else {
        Err(v
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or("YouTube refused")
            .to_string())
    }
}

pub fn my_channel(token: &str) -> Result<String, String> {
    let v = api_get("channels?part=snippet&mine=true", token)?;
    Ok(v.pointer("/items/0/snippet/title")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string())
}

/// Sends a chat message to a live video's chat.
pub fn send(saved: &YouTubeSaved, video_id: &str, text: &str) -> Result<(), String> {
    let token = access_token(saved)?;
    let v = api_get(
        &format!("videos?part=liveStreamingDetails&id={video_id}"),
        &token,
    )?;
    let chat = v
        .pointer("/items/0/liveStreamingDetails/activeLiveChatId")
        .and_then(Value::as_str)
        .ok_or("This stream has no live chat")?
        .to_string();
    let body = serde_json::json!({
        "snippet": {"liveChatId": chat, "type": "textMessageEvent", "textMessageDetails": {"messageText": text}}
    });
    let mut r = agent()
        .post(&format!("{API}liveChat/messages?part=snippet"))
        .header("Authorization", &format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .send(body.to_string())
        .map_err(|e| e.to_string())?;
    if r.status() == 200 {
        Ok(())
    } else {
        let v: Value = r.body_mut().read_json().unwrap_or(Value::Null);
        Err(v
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or("YouTube refused the message")
            .to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_and_url() {
        let (v, c) = pkce();
        assert!(v.len() >= 43);
        assert_eq!(c.len(), 43); // 32-byte SHA-256 in base64url without padding
        let u = auth_url(
            "id.apps.googleusercontent.com",
            "http://127.0.0.1:5555",
            "st",
            &c,
        );
        assert!(u.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A5555"));
        assert!(u.contains("code_challenge_method=S256"));
        assert!(u.contains("youtube.force-ssl"));
    }

    #[test]
    fn sapisid_hash_form() {
        let s = crate::weblogin::WebSession {
            cookies: vec![("SAPISID".into(), "abc".into())],
            user_agent: String::new(),
        };
        let h = sapisid_hash(&s, 1700000000).unwrap();
        assert_eq!(
            h,
            format!(
                "SAPISIDHASH 1700000000_{}",
                sha1_hex("1700000000 abc https://www.youtube.com")
            )
        );
        assert_eq!(sha1_hex("abc"), "a9993e364706816aba3e25717850c26c9cd0d89d"); // FIPS 180 example
        assert!(sapisid_hash(&Default::default(), 1).is_none());
    }

    #[test]
    fn known_sha256_base64url() {
        // RFC 7636 appendix B example
        let c = b64url(&sha2::Sha256::digest(
            b"dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
        ));
        assert_eq!(c, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    /// The loopback listener catches the code and checks the state.
    #[test]
    fn loopback_catches_code() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let h = std::thread::spawn(move || wait_for_code(l, "abc"));
        std::thread::sleep(Duration::from_millis(200));
        let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(
            s,
            "GET /?state=abc&code=4%2F0Axyz HTTP/1.1\r\nHost: x\r\n\r\n"
        )
        .unwrap();
        let mut resp = String::new();
        let _ = s.read_to_string(&mut resp);
        assert_eq!(h.join().unwrap().unwrap(), "4/0Axyz");
        assert!(resp.contains("Signed in"));
    }
}
