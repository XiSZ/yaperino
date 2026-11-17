//! Twitch accounts, signed in the way chatterino7 and Moltorino do it:
//!
//! 1. Device login (Moltorino): Twitch's device-code flow
//!    (`POST https://id.twitch.tv/oauth2/device`, then polling
//!    `oauth2/token` with `urn:ietf:params:oauth:grant-type:device_code`). The app
//!    opens Twitch Activate and copies the code for you. Tokens are renewed with
//!    the refresh token (`grant_type=refresh_token`).
//! 2. Paste login info (Chatterino's LoginDialog): `oauth_token=...;client_id=...;
//!    username=...;user_id=...`.
//! 3. Advanced (Chatterino): username, user ID, client ID and OAuth token by hand.
//!
//! Every account is checked with `oauth2/validate`. Several accounts can be saved;
//! one is current. Moltorino's own sign-in is not used.

use serde_json::Value;
use std::time::Duration;

/// The permissions asked for: the part of Moltorino's scope list (its
/// TwitchAccount scope table) that this app uses.
pub const SCOPES: &str = "chat:read chat:edit whispers:read user:read:follows moderator:read:followers user:read:chat user:write:chat user:read:emotes user:read:blocked_users user:manage:blocked_users user:manage:chat_color moderator:manage:banned_users moderator:manage:chat_messages moderator:manage:chat_settings moderator:manage:warnings moderator:manage:shield_mode moderator:manage:announcements moderator:manage:shoutouts moderator:manage:blocked_terms moderator:manage:automod moderator:manage:unban_requests moderator:read:moderators moderator:read:vips channel:manage:moderators channel:manage:vips channel:manage:raids channel:edit:commercial channel:manage:broadcast channel:manage:polls channel:manage:predictions";
/// The Client ID of this app's registered Twitch application (Public client), given by
/// the person; device login works with no setup. Settings can override it.
/// Built in from `twitch_client_id.txt` next to Cargo.toml (or YAPERINO_TWITCH_CLIENT_ID) by
/// build.rs; that file is not published. Empty: Settings > Accounts asks for a Client ID.
pub const DEFAULT_CLIENT_ID: &str = match option_env!("YAPERINO_TWITCH_CLIENT_ID") {
    Some(id) => id,
    None => "",
};
const VALIDATE_URL: &str = "https://id.twitch.tv/oauth2/validate";
const DEVICE_URL: &str = "https://id.twitch.tv/oauth2/device";
const TOKEN_URL: &str = "https://id.twitch.tv/oauth2/token";

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Source {
    /// Kept so older saved settings still load; no longer used.
    Moltorino,
    Device,
}

#[derive(Clone)]
pub struct Account {
    pub source: Source,
    pub login: String,
    pub user_id: String,
    pub client_id: String,
    pub token: String,
    #[allow(dead_code)]
    pub refresh: Option<String>,
    pub scopes: Vec<String>,
}

impl Account {
    /// The sign-in allows timeouts, bans and deleting messages (in channels you moderate).
    pub fn can_moderate(&self) -> bool {
        self.scopes
            .iter()
            .any(|s| s == "moderator:manage:banned_users")
    }

    /// Permissions Yaperino asks for that this sign-in doesn't have yet (signed in before the
    /// mod tools were added): sign in again to grant them.
    pub fn missing_scopes(&self) -> Vec<&'static str> {
        if self.scopes.is_empty() {
            // a pasted token: its permissions aren't known
            return Vec::new();
        }
        SCOPES
            .split(' ')
            .filter(|s| !self.scopes.iter().any(|h| h == s))
            .collect()
    }

    pub fn has_scope(&self, s: &str) -> bool {
        self.scopes.iter().any(|h| h == s)
    }

    pub fn can_follow(&self) -> bool {
        self.scopes.iter().any(|s| s == "user:read:follows")
    }
    pub fn can_chat(&self) -> bool {
        self.scopes.iter().any(|s| s == "chat:edit")
    }
}

impl std::fmt::Debug for Account {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // never print the token
        f.debug_struct("Account")
            .field("login", &self.login)
            .field("source", &self.source)
            .finish()
    }
}

/// One saved Twitch account (this app's own; Chatterino keeps the same fields).
#[derive(Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SavedDevice {
    pub client_id: String,
    pub token: String,
    /// Empty for pasted/manual logins (they can't be renewed; sign in again when they end).
    pub refresh: String,
    pub username: String,
    pub user_id: String,
}

impl std::fmt::Debug for SavedDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SavedDevice")
            .field("username", &self.username)
            .finish()
    }
}

/// Chatterino's "Paste login info": `oauth_token=..;client_id=..;username=..;user_id=..`.
pub fn parse_login_info(text: &str) -> Option<SavedDevice> {
    let mut s = SavedDevice::default();
    for part in text.trim().split(';') {
        let Some((k, v)) = part.split_once('=') else {
            continue;
        };
        let v = v.trim().to_string();
        match k.trim() {
            "oauth_token" => s.token = v.trim_start_matches("oauth:").to_string(),
            "client_id" => s.client_id = v,
            "username" => s.username = v.to_lowercase(),
            "user_id" => s.user_id = v,
            _ => {}
        }
    }
    (!s.token.is_empty() && !s.client_id.is_empty()).then_some(s)
}

/// Checks a new login with Twitch and fills in its name, id and client id.
pub fn complete(mut saved: SavedDevice) -> Result<SavedDevice, String> {
    match validate(&saved.token) {
        Check::Valid(v) => {
            if !saved.client_id.is_empty()
                && !v.client_id.is_empty()
                && saved.client_id != v.client_id
            {
                return Err("The client ID doesn't belong to this OAuth token".into());
            }
            saved.username = v.login;
            saved.user_id = v.user_id;
            if saved.client_id.is_empty() {
                saved.client_id = v.client_id;
            }
            Ok(saved)
        }
        Check::Invalid => Err("Twitch says this OAuth token is not valid".into()),
        Check::Error(e) => Err(format!("Couldn't check the sign-in: {e}")),
    }
}
pub fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .http_status_as_error(false)
        .user_agent("Yaperino/0.2")
        .build()
        .into()
}

pub struct Validated {
    pub login: String,
    pub user_id: String,
    pub client_id: String,
    pub scopes: Vec<String>,
    #[allow(dead_code)]
    pub expires_in: i64,
}

pub enum Check {
    Valid(Validated),
    Invalid,
    Error(String),
}

pub fn validate(token: &str) -> Check {
    let resp = agent()
        .get(VALIDATE_URL)
        .header("Authorization", &format!("OAuth {token}"))
        .call();
    match resp {
        Ok(mut r) => {
            if r.status() == 401 {
                return Check::Invalid;
            }
            match r.body_mut().read_json::<Value>() {
                Ok(v) if v.get("login").is_some() => {
                    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
                    Check::Valid(Validated {
                        login: s("login"),
                        user_id: s("user_id"),
                        client_id: s("client_id"),
                        scopes: v
                            .get("scopes")
                            .and_then(Value::as_array)
                            .map(|a| {
                                a.iter()
                                    .filter_map(|x| x.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        expires_in: v.get("expires_in").and_then(Value::as_i64).unwrap_or(0),
                    })
                }
                Ok(_) => Check::Invalid,
                Err(e) => Check::Error(e.to_string()),
            }
        }
        Err(e) => Check::Error(e.to_string()),
    }
}

/// Device sign-in, step 1: returns (device_code, user_code, verification_uri, interval seconds).
pub fn device_start(client_id: &str) -> Result<(String, String, String, u64), String> {
    let mut r = agent()
        .post(DEVICE_URL)
        .send_form([("client_id", client_id), ("scopes", SCOPES)])
        .map_err(|e| e.to_string())?;
    let v: Value = r.body_mut().read_json().map_err(|e| e.to_string())?;
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    if s("device_code").is_empty() {
        return Err(v
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Twitch refused this Client ID")
            .to_string());
    }
    let interval = v.get("interval").and_then(Value::as_u64).unwrap_or(5);
    Ok((
        s("device_code"),
        s("user_code"),
        s("verification_uri"),
        interval,
    ))
}

pub enum Poll {
    Pending,
    Done(SavedDevice),
    Failed(String),
}

/// Device sign-in, step 2: ask whether the person approved it yet.
pub fn device_poll(client_id: &str, device_code: &str) -> Poll {
    let r = agent().post(TOKEN_URL).send_form([
        ("client_id", client_id),
        ("scopes", SCOPES),
        ("device_code", device_code),
        ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
    ]);
    let mut r = match r {
        Ok(r) => r,
        Err(e) => return Poll::Failed(e.to_string()),
    };
    let v: Value = match r.body_mut().read_json() {
        Ok(v) => v,
        Err(e) => return Poll::Failed(e.to_string()),
    };
    if let Some(tok) = v.get("access_token").and_then(Value::as_str) {
        return Poll::Done(SavedDevice {
            client_id: client_id.to_string(),
            token: tok.to_string(),
            refresh: v
                .get("refresh_token")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            ..Default::default()
        });
    }
    let msg = v.get("message").and_then(Value::as_str).unwrap_or("");
    if msg.contains("authorization_pending") || msg.contains("slow_down") {
        Poll::Pending
    } else {
        Poll::Failed(if msg.is_empty() {
            "sign-in failed".into()
        } else {
            msg.to_string()
        })
    }
}

/// Renews a device-login token with its refresh token.
pub fn device_refresh(saved: &SavedDevice) -> Result<SavedDevice, String> {
    let mut r = agent()
        .post(TOKEN_URL)
        .send_form([
            ("client_id", saved.client_id.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", saved.refresh.as_str()),
        ])
        .map_err(|e| e.to_string())?;
    let v: Value = r.body_mut().read_json().map_err(|e| e.to_string())?;
    match v.get("access_token").and_then(Value::as_str) {
        Some(t) => Ok(SavedDevice {
            token: t.to_string(),
            refresh: v
                .get("refresh_token")
                .and_then(Value::as_str)
                .unwrap_or(&saved.refresh)
                .to_string(),
            ..saved.clone()
        }),
        None => Err("Twitch sign-in expired; please sign in again".into()),
    }
}

/// Turns a saved device sign-in into an account, renewing it when needed.
pub fn from_device(saved: &SavedDevice) -> Result<(Account, SavedDevice), String> {
    let mut saved = saved.clone();
    let v = match validate(&saved.token) {
        Check::Valid(v) => v,
        Check::Invalid if saved.refresh.is_empty() => {
            return Err(format!(
                "The sign-in for {} has ended; sign in again",
                saved.username
            ));
        }
        Check::Invalid => {
            saved = device_refresh(&saved)?;
            match validate(&saved.token) {
                Check::Valid(v) => v,
                _ => return Err("Twitch sign-in expired; please sign in again".into()),
            }
        }
        Check::Error(e) => return Err(format!("Couldn't check the sign-in: {e}")),
    };
    saved.username = v.login.clone();
    saved.user_id = v.user_id.clone();
    Ok((
        Account {
            source: Source::Device,
            login: v.login,
            user_id: v.user_id,
            client_id: saved.client_id.clone(),
            token: saved.token.clone(),
            refresh: Some(saved.refresh.clone()),
            scopes: v.scopes,
        },
        saved,
    ))
}

/// For network tests: an account from LIVE_STREAMS_TEST_LOGIN (Chatterino login-info format).
#[cfg(test)]
pub fn test_account() -> Option<Account> {
    let s = parse_login_info(&std::env::var("LIVE_STREAMS_TEST_LOGIN").ok()?)?;
    from_device(&s).ok().map(|(a, _)| a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_chatterino_login_info() {
        let s =
            parse_login_info("oauth_token=oauth:abc123;client_id=cid;username=SomeOne;user_id=42")
                .unwrap();
        assert_eq!(
            (
                s.token.as_str(),
                s.client_id.as_str(),
                s.username.as_str(),
                s.user_id.as_str()
            ),
            ("abc123", "cid", "someone", "42")
        );
        assert!(parse_login_info("nothing here").is_none());
    }
}
