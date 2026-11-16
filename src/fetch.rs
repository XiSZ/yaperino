//! Where the channel list comes from and how "who is live" is asked.
//!
//! The list is read from the person's own "Online Channels" plugin data
//! (the same file their /online command uses), plus channels added in this
//! window. Live state comes from api.ivr.fi in batches of 50 logins, the way
//! that plugin already does it successfully (see MECHANICS.md).

use crate::model::{parse_iso8601, Stream};
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

pub const BATCH: usize = 50;
pub const TIMEOUT_MS: u64 = 10_000;
pub const IVR_URL: &str = "https://api.ivr.fi/v2/twitch/user?login=";

const NAME_KEYS: [&str; 6] = [
    "login",
    "broadcaster_login",
    "user_login",
    "to_login",
    "name",
    "channel",
];
const LIST_KEYS: [&str; 4] = ["data", "channels", "follows", "following"];

/// Default place of the person's plugin channel list.
pub fn default_channels_file() -> Option<PathBuf> {
    // Chatterino's own folder: %APPDATA%\Chatterino2, ~/.local/share/chatterino,
    // ~/Library/Application Support/chatterino
    #[cfg(windows)]
    let chatterino = PathBuf::from(std::env::var_os("APPDATA")?).join("Chatterino2");
    #[cfg(target_os = "macos")]
    let chatterino = std::env::home_dir()?
        .join("Library")
        .join("Application Support")
        .join("chatterino");
    #[cfg(not(any(windows, target_os = "macos")))]
    let chatterino = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::home_dir().map(|h| h.join(".local").join("share")))?
        .join("chatterino");
    Some(
        chatterino
            .join("Plugins")
            .join("online-channels")
            .join("data")
            .join("channels.json"),
    )
}

pub fn valid_login(name: &str) -> Option<String> {
    let n = name.trim().trim_start_matches('#').to_lowercase();
    if !n.is_empty() && n.len() <= 25 && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        Some(n)
    } else {
        None
    }
}

fn collect(value: &Value, out: &mut Vec<String>, seen: &mut HashSet<String>) {
    match value {
        Value::String(s) => {
            if let Some(n) = valid_login(s) {
                if seen.insert(n.clone()) {
                    out.push(n);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                collect(item, out, seen);
            }
        }
        Value::Object(map) => {
            for key in NAME_KEYS {
                if let Some(v @ Value::String(_)) = map.get(key) {
                    collect(v, out, seen);
                    return;
                }
            }
            for key in LIST_KEYS {
                if let Some(v) = map.get(key) {
                    collect(v, out, seen);
                }
            }
        }
        _ => {}
    }
}

/// Reads logins from a JSON file of any of the shapes the plugin accepts,
/// or a plain text file with one name per line.
pub fn read_channel_file(path: &Path) -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    match serde_json::from_str::<Value>(&text) {
        Ok(v) => collect(&v, &mut out, &mut seen),
        Err(_) => {
            for line in text.lines() {
                collect(&Value::String(line.to_string()), &mut out, &mut seen);
            }
        }
    }
    Ok(out)
}

pub fn merge_lists(primary: Vec<String>, extra: &[String]) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for n in extra.iter().cloned().chain(primary) {
        if seen.insert(n.clone()) {
            out.push(n);
        }
    }
    out
}

fn stream_from_user(user: &Value, index: usize) -> Option<Stream> {
    let login = user.get("login")?.as_str()?.to_string();
    let stream = user.get("stream")?;
    if !stream.is_object() {
        return None; // offline: IVR sends null
    }
    let display_name = user
        .get("displayName")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or(&login)
        .to_string();
    let title = stream
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let game = stream
        .get("game")
        .and_then(|g| g.get("displayName").or_else(|| g.get("name")))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let viewers = stream
        .get("viewersCount")
        .and_then(|v| {
            v.as_u64()
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        })
        .unwrap_or(0);
    let started = stream
        .get("createdAt")
        .and_then(Value::as_str)
        .and_then(parse_iso8601)
        .unwrap_or_else(crate::model::now_unix);
    Some(Stream {
        group: login.clone(),
        login,
        display_name,
        title,
        game,
        viewers,
        started,
        status_changed: started,
        list_index: index,
        platform: crate::platforms::Platform::Twitch,
    })
}

pub struct FetchResult {
    pub streams: Vec<Stream>,
    pub error: Option<String>,
    /// How many live streams came from the account's follows (None = not signed in).
    pub followed: Option<usize>,
    /// Twitch refused the account's token (401): load the sign-in again.
    pub auth_failed: bool,
    /// Kick / YouTube / TikTok results for linked channels.
    pub other: Option<crate::platforms::OtherLive>,
    /// Every channel the account follows (live or not), when signed in.
    pub follows: Option<Vec<String>>,
}

/// The signed-in account, for Helix calls.
#[derive(Clone)]
pub struct Auth {
    pub client_id: String,
    pub token: String,
    pub user_id: String,
}

pub const HELIX_FOLLOWED: &str = "https://api.twitch.tv/helix/streams/followed";
pub const HELIX_PAGE: usize = 100;

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_millis(TIMEOUT_MS)))
        .http_status_as_error(false)
        .user_agent("Yaperino/0.2")
        .build()
        .into()
}

fn stream_from_helix(v: &Value, index: usize) -> Option<Stream> {
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let login = s("user_login");
    if login.is_empty() {
        return None;
    }
    let name = s("user_name");
    Some(Stream {
        display_name: if name.is_empty() { login.clone() } else { name },
        login,
        title: s("title").trim().to_string(),
        game: s("game_name"),
        viewers: v.get("viewer_count").and_then(Value::as_u64).unwrap_or(0),
        started: parse_iso8601(&s("started_at")).unwrap_or_else(crate::model::now_unix),
        status_changed: 0,
        list_index: index,
        platform: crate::platforms::Platform::Twitch,
        group: String::new(),
    })
}

/// Every live stream the account follows (all pages). Err(true) = token refused.
fn fetch_followed(agent: &ureq::Agent, auth: &Auth) -> Result<Vec<Stream>, (bool, String)> {
    let mut out = Vec::new();
    let mut cursor = String::new();
    for _ in 0..50 {
        let mut url = format!(
            "{HELIX_FOLLOWED}?user_id={}&first={HELIX_PAGE}",
            auth.user_id
        );
        if !cursor.is_empty() {
            url.push_str("&after=");
            url.push_str(&cursor);
        }
        let mut r = agent
            .get(&url)
            .header("Authorization", &format!("Bearer {}", auth.token))
            .header("Client-Id", &auth.client_id)
            .call()
            .map_err(|e| (false, e.to_string()))?;
        if r.status() == 401 {
            return Err((true, "Twitch refused the sign-in".into()));
        }
        if r.status() != 200 {
            return Err((false, format!("Twitch answered {}", r.status())));
        }
        let v: Value = r
            .body_mut()
            .read_json()
            .map_err(|e| (false, e.to_string()))?;
        if let Some(items) = v.get("data").and_then(Value::as_array) {
            for item in items {
                if let Some(s) = stream_from_helix(item, out.len()) {
                    out.push(s);
                }
            }
        }
        cursor = v
            .get("pagination")
            .and_then(|p| p.get("cursor"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if cursor.is_empty() {
            break;
        }
    }
    Ok(out)
}

/// Live state for the list file and added channels, plus (signed in) every
/// followed live stream. Duplicates are merged; the Twitch (Helix) data wins.
pub fn fetch_live(list: &[String], auth: Option<&Auth>) -> FetchResult {
    let agent = agent();
    let mut streams: Vec<Stream> = Vec::new();
    let mut error = None;
    let mut followed = None;
    let mut auth_failed = false;
    if let Some(a) = auth {
        match fetch_followed(&agent, a) {
            Ok(f) => {
                followed = Some(f.len());
                streams = f;
            }
            Err((refused, e)) => {
                auth_failed = refused;
                error = Some(e);
            }
        }
    }
    let have: HashSet<String> = streams.iter().map(|s| s.login.clone()).collect();
    let rest: Vec<String> = list
        .iter()
        .filter(|l| !have.contains(*l))
        .cloned()
        .collect();
    let position: std::collections::HashMap<&str, usize> = list
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i))
        .collect();
    for batch in rest.chunks(BATCH) {
        let url = format!("{IVR_URL}{}", batch.join(","));
        let answer = agent
            .get(&url)
            .call()
            .map_err(|e| e.to_string())
            .and_then(|mut r| r.body_mut().read_json::<Value>().map_err(|e| e.to_string()));
        match answer {
            Ok(Value::Array(users)) => {
                for u in &users {
                    let idx = u
                        .get("login")
                        .and_then(Value::as_str)
                        .and_then(|l| position.get(l).copied())
                        .unwrap_or(usize::MAX);
                    if let Some(s) = stream_from_user(u, idx) {
                        if !have.contains(&s.login) {
                            streams.push(s);
                        }
                    }
                }
            }
            Ok(_) => error = Some("unexpected answer from api.ivr.fi".into()),
            Err(e) => error = Some(e),
        }
    }
    FetchResult {
        streams,
        error,
        followed,
        auth_failed,
        other: None,
        follows: None,
    }
}

pub const HELIX_FOLLOWS: &str = "https://api.twitch.tv/helix/channels/followed";
/// The follow list (live or not) is asked again after this long.
pub const FOLLOWS_EVERY: Duration = Duration::from_secs(15 * 60);

/// Every channel the account follows (live or not), for finding them on other platforms by name.
pub fn fetch_follow_list(agent: &ureq::Agent, auth: &Auth) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut cursor = String::new();
    for _ in 0..100 {
        let mut url = format!(
            "{HELIX_FOLLOWS}?user_id={}&first={HELIX_PAGE}",
            auth.user_id
        );
        if !cursor.is_empty() {
            url.push_str("&after=");
            url.push_str(&cursor);
        }
        let mut r = agent
            .get(&url)
            .header("Authorization", &format!("Bearer {}", auth.token))
            .header("Client-Id", &auth.client_id)
            .call()
            .map_err(|e| e.to_string())?;
        if r.status() != 200 {
            return Err(format!("Twitch answered {}", r.status()));
        }
        let v: Value = r.body_mut().read_json().map_err(|e| e.to_string())?;
        if let Some(items) = v.get("data").and_then(Value::as_array) {
            out.extend(
                items
                    .iter()
                    .filter_map(|i| i.get("broadcaster_login").and_then(Value::as_str))
                    .filter_map(valid_login),
            );
        }
        cursor = v
            .pointer("/pagination/cursor")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if cursor.is_empty() {
            break;
        }
    }
    Ok(out)
}

pub enum Job {
    /// Channel list, Twitch account, channels on other platforms (group -> links), and the
    /// groups with an open tab.
    Check(
        Vec<String>,
        Option<Auth>,
        Vec<(String, crate::platforms::Links)>,
        HashSet<String>,
    ),
}

/// A background worker so the window never freezes while asking.
pub fn spawn_worker(ctx: egui::Context) -> (Sender<Job>, Receiver<FetchResult>) {
    let (job_tx, job_rx) = std::sync::mpsc::channel::<Job>();
    let (res_tx, res_rx) = std::sync::mpsc::channel::<FetchResult>();
    std::thread::spawn(move || {
        let mut checker = crate::platforms::LinkChecker::default();
        let mut follows: Option<(std::time::Instant, String, Vec<String>)> = None;
        while let Ok(Job::Check(list, auth, links, open)) = job_rx.recv() {
            // other platforms in parallel with Twitch
            let other = std::thread::scope(|s| {
                let h = (!links.is_empty()).then(|| s.spawn(|| checker.check(&links, &open)));
                let mut r = fetch_live(&list, auth.as_ref());
                // the whole follow list, now and then (for name matching on other platforms)
                if let Some(a) = &auth {
                    let stale = follows.as_ref().is_none_or(|(at, user, _)| {
                        *user != a.user_id || at.elapsed() >= FOLLOWS_EVERY
                    });
                    if stale {
                        if let Ok(f) = fetch_follow_list(&agent(), a) {
                            follows = Some((std::time::Instant::now(), a.user_id.clone(), f));
                        }
                    }
                    r.follows = follows
                        .as_ref()
                        .filter(|(_, u, _)| *u == a.user_id)
                        .map(|(_, _, f)| f.clone());
                }
                (h.and_then(|h| h.join().ok()), r)
            });
            let (other, mut r) = other;
            if let Some(o) = other {
                r.streams.extend(o.streams.iter().cloned());
                r.other = Some(o);
            }
            if res_tx.send(r).is_err() {
                break;
            }
            ctx.request_repaint();
        }
    });
    (job_tx, res_rx)
}

#[cfg(test)]
mod tests {
    /// Network test with the Moltorino account: counts followed live streams.
    /// `cargo test -- --ignored followed_streams --nocapture`
    #[test]
    #[ignore]
    fn followed_streams() {
        let Some(acc) = crate::account::test_account() else {
            println!("set LIVE_STREAMS_TEST_LOGIN to run this test");
            return;
        };
        let auth = super::Auth {
            client_id: acc.client_id,
            token: acc.token,
            user_id: acc.user_id,
        };
        let r = super::fetch_live(&[], Some(&auth));
        println!(
            "followed live: {:?}, error: {:?}, refused: {}",
            r.followed, r.error, r.auth_failed
        );
        assert!(r.followed.is_some());
    }
}
