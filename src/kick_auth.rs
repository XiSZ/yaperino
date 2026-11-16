//! Kick sign-in with the person's own Kick app, as Moltorino's KickLoginPage
//! "manual" method does: OAuth 2.1 with PKCE (S256) and a `state` check.
//! - authorize: `https://id.kick.com/oauth/authorize` (response_type=code)
//! - the browser comes back to `http://localhost:38275` (add this redirect URL to the
//!   app in kick.com/settings/developer)
//! - token: `POST https://id.kick.com/oauth/token` (authorization_code, then
//!   refresh_token), with the app's client secret
//! - user: `GET https://api.kick.com/public/v1/users`
//! Moltorino's other method (chatterino7's login service) belongs to chatterino7 and
//! is not used. Moltorino's Kick sign-in is not read.

use serde_json::Value;
use std::net::TcpListener;

pub const AUTHORIZE: &str = "https://id.kick.com/oauth/authorize";
pub const TOKEN: &str = "https://id.kick.com/oauth/token";
pub const USERS: &str = "https://api.kick.com/public/v1/users";
pub const REDIRECT_PORT: u16 = 38275;
pub const REDIRECT: &str = "http://localhost:38275";
/// Moltorino's Kick scopes (KickLoginPage.cpp).
pub const SCOPES: &str =
    "user:read channel:read channel:write chat:write moderation:ban moderation:chat_message:manage";

#[derive(Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct KickSaved {
    pub client_id: String,
    pub client_secret: String,
    pub token: String,
    pub refresh: String,
    /// Unix seconds the token ends.
    pub expires_at: i64,
    pub username: String,
    pub user_id: u64,
    /// Signed in on kick.com itself in the sign-in window (no developer app needed).
    pub session: crate::weblogin::WebSession,
}

impl KickSaved {
    pub fn signed_in(&self) -> bool {
        !self.token.is_empty() || self.session.get("session_token").is_some()
    }
}

// ---------------------------------------------------------------- kick.com sign-in (no app)
//
// The way KickTalk and kick.com's own page talk to Kick: the `session_token` cookie as a
// Bearer token, with the site's cookies and its XSRF token.

pub const WEB_USER: &str = "https://kick.com/api/v1/user";
pub const WEB_SEND: &str = "https://kick.com/api/v2/messages/send/";

fn web_headers(s: &crate::weblogin::WebSession) -> Vec<(&'static str, String)> {
    let mut h = vec![
        ("Accept", "application/json".to_string()),
        ("Cookie", s.header()),
    ];
    if let Some(t) = s.get("session_token") {
        h.push((
            "Authorization",
            format!("Bearer {}", crate::weblogin::url_decode(t)),
        ));
    }
    if let Some(x) = s.get("XSRF-TOKEN") {
        h.push(("X-XSRF-TOKEN", crate::weblogin::url_decode(x)));
    }
    h
}

/// Who is signed in: (username, user id). Only reads.
pub fn web_whoami(s: &crate::weblogin::WebSession) -> Result<(String, u64), String> {
    let h = web_headers(s);
    let hr: Vec<(&str, &str)> = h.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let (status, body) = crate::platforms::curl_as("GET", WEB_USER, &hr, None, &s.user_agent)?;
    if status != 200 {
        return Err(format!("Kick answered {status}"));
    }
    let v: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let name = v
        .get("username")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let id = v.get("id").and_then(Value::as_u64).unwrap_or(0);
    if name.is_empty() {
        return Err("Kick didn't say who is signed in".into());
    }
    Ok((name, id))
}

/// Sends to a chatroom with the kick.com sign-in.
pub fn web_send(
    s: &crate::weblogin::WebSession,
    chatroom_id: u64,
    text: &str,
) -> Result<(), String> {
    let mut h = web_headers(s);
    h.push(("Content-Type", "application/json".into()));
    let hr: Vec<(&str, &str)> = h.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let body = serde_json::json!({ "content": text, "type": "message" }).to_string();
    let (status, answer) = crate::platforms::curl_as(
        "POST",
        &format!("{WEB_SEND}{chatroom_id}"),
        &hr,
        Some(&body),
        &s.user_agent,
    )?;
    match status {
        200 | 201 => Ok(()),
        401 | 419 => Err("Kick ended the sign-in: sign in again (Settings > Accounts)".into()),
        _ => {
            let v: Value = serde_json::from_str(&answer).unwrap_or(Value::Null);
            Err(v
                .pointer("/status/message")
                .or_else(|| v.get("message"))
                .and_then(Value::as_str)
                .map(String::from)
                .unwrap_or_else(|| format!("Kick answered {status}")))
        }
    }
}

impl std::fmt::Debug for KickSaved {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KickSaved")
            .field("username", &self.username)
            .finish()
    }
}

fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn form(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", enc(k), enc(v)))
        .collect::<Vec<_>>()
        .join("&")
}

pub fn authorize_url(client_id: &str, state: &str, challenge: &str) -> String {
    format!(
        "{AUTHORIZE}?{}",
        form(&[
            ("response_type", "code"),
            ("client_id", client_id),
            ("redirect_uri", REDIRECT),
            ("scope", SCOPES),
            ("code_challenge", challenge),
            ("code_challenge_method", "S256"),
            ("state", state),
        ])
    )
}

/// Posts a form whose body holds secrets: through curl.exe with the body on stdin
/// (never on the command line), as Kick's edge refuses Rust's TLS.
fn post_form(url: &str, body: &str) -> Result<Value, String> {
    let (status, answer) =
        crate::platforms::curl_secret_body(url, "application/x-www-form-urlencoded", body)?;
    let v: Value = serde_json::from_str(&answer).unwrap_or(Value::Null);
    if status == 200 && v.get("access_token").is_some() {
        Ok(v)
    } else {
        Err(v
            .get("error_description")
            .or_else(|| v.get("message"))
            .or_else(|| v.get("error"))
            .and_then(Value::as_str)
            .map(String::from)
            .unwrap_or_else(|| format!("Kick answered {status}")))
    }
}

fn take_tokens(saved: &mut KickSaved, v: &Value) {
    saved.token = v
        .get("access_token")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if let Some(r) = v.get("refresh_token").and_then(Value::as_str) {
        saved.refresh = r.to_string();
    }
    let ttl = v.get("expires_in").and_then(Value::as_i64).unwrap_or(3600);
    saved.expires_at = crate::model::now_unix() + ttl - 60;
}

fn fetch_user(saved: &mut KickSaved) -> Result<(), String> {
    let auth = format!("Bearer {}", saved.token);
    let (status, body) = crate::platforms::curl_full(
        "GET",
        USERS,
        &[("Authorization", auth.as_str())],
        None,
        false,
    )
    .unwrap_or_else(|| Err("curl.exe not found".into()))
    .map(|(s, _, b)| (s, b))?;
    if status != 200 {
        return Err(format!("Kick user lookup: {status}"));
    }
    let v: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let u = v.pointer("/data/0").cloned().unwrap_or(Value::Null);
    saved.username = u
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    saved.user_id = u.get("user_id").and_then(Value::as_u64).unwrap_or(0);
    Ok(())
}

/// The whole sign-in: browser, wait for the return to localhost:38275, exchange, user.
pub fn sign_in(
    client_id: &str,
    client_secret: &str,
    open_browser: impl Fn(&str),
) -> Result<KickSaved, String> {
    let listener = TcpListener::bind(("127.0.0.1", REDIRECT_PORT)).map_err(|e| {
        format!("Port {REDIRECT_PORT} is busy (is Moltorino's Kick sign-in open?): {e}")
    })?;
    let (verifier, challenge) = crate::youtube_auth::pkce();
    let state = crate::youtube_auth::random_state();
    open_browser(&authorize_url(client_id, &state, &challenge));
    let code = crate::youtube_auth::wait_for_code(listener, &state)?;
    let v = post_form(
        TOKEN,
        &form(&[
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("redirect_uri", REDIRECT),
            ("code_verifier", &verifier),
            ("code", &code),
        ]),
    )?;
    let mut saved = KickSaved {
        client_id: client_id.into(),
        client_secret: client_secret.into(),
        ..Default::default()
    };
    take_tokens(&mut saved, &v);
    fetch_user(&mut saved)?;
    Ok(saved)
}

/// A usable token: renewed with the refresh token when it has ended.
pub fn fresh(saved: &KickSaved) -> Result<KickSaved, String> {
    let mut s = saved.clone();
    if s.token.is_empty() {
        return Err("Not signed in to Kick".into());
    }
    if crate::model::now_unix() < s.expires_at {
        return Ok(s);
    }
    if s.refresh.is_empty() {
        return Err("The Kick sign-in has ended; sign in again".into());
    }
    let v = post_form(
        TOKEN,
        &form(&[
            ("grant_type", "refresh_token"),
            ("client_id", &s.client_id),
            ("client_secret", &s.client_secret),
            ("refresh_token", &s.refresh),
        ]),
    )?;
    take_tokens(&mut s, &v);
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorize_url_has_pkce_and_redirect() {
        let u = authorize_url("cid", "st", "chal");
        assert!(
            u.starts_with("https://id.kick.com/oauth/authorize?response_type=code&client_id=cid")
        );
        assert!(u.contains("redirect_uri=http%3A%2F%2Flocalhost%3A38275"));
        assert!(u.contains("code_challenge_method=S256"));
        assert!(u.contains("scope=user%3Aread%20channel%3Aread"));
    }
}
