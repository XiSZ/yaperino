//! Twitch EventSub over WebSocket, for the channels the signed-in account moderates:
//! AutoMod's held messages (automod.message.hold / .update v2), unban requests
//! (channel.unban_request.create / .resolve v1) and every moderation action
//! (channel.moderate v2, for the mod log). All official Twitch API.
//!
//! One background connection; it only opens while there's a channel to watch (Twitch
//! closes a connection that has no subscription after 10 seconds). Each kind is only
//! subscribed when the sign-in has its permissions, so older sign-ins keep working and
//! are asked to sign in again. After any drop it connects fresh and subscribes again.

use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::net::TcpStream;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::Message;

const URL: &str = "wss://eventsub.wss.twitch.tv/ws?keepalive_timeout_seconds=30";

/// What is subscribed per channel: (type, version, the permissions it needs).
const KINDS: &[(&str, &str, &[&str])] = &[
    ("automod.message.hold", "2", &["moderator:manage:automod"]),
    ("automod.message.update", "2", &["moderator:manage:automod"]),
    (
        "channel.unban_request.create",
        "1",
        &["moderator:manage:unban_requests"],
    ),
    (
        "channel.unban_request.resolve",
        "1",
        &["moderator:manage:unban_requests"],
    ),
    (
        "channel.moderate",
        "2",
        &[
            "moderator:manage:blocked_terms",
            "moderator:manage:chat_settings",
            "moderator:manage:unban_requests",
            "moderator:manage:banned_users",
            "moderator:manage:chat_messages",
            "moderator:manage:warnings",
            "moderator:read:moderators",
            "moderator:read:vips",
        ],
    ),
];

/// The sign-in the connection subscribes with.
#[derive(Clone, PartialEq)]
pub struct EsAuth {
    pub client_id: String,
    pub token: String,
    pub user_id: String,
    /// The sign-in's permissions (empty: unknown, everything is tried).
    pub scopes: Vec<String>,
}

pub enum EsCmd {
    Auth(Option<EsAuth>),
    /// Watch a channel: (login, Twitch id).
    Watch(String, String),
    Unwatch(String),
}

/// What happened, per channel login.
#[derive(Debug, Clone, PartialEq)]
pub enum EsEvent {
    /// AutoMod held a message.
    Held(String, crate::chat::HeldMessage),
    /// A held message was handled: (channel, message id, status, moderator).
    HeldDone(String, String, String, String),
    UnbanRequest(String, crate::moderation::UnbanRequest),
    /// An unban request was handled: (channel, request id, status, moderator).
    UnbanResolved(String, String, String, String),
    /// A moderation action, for the mod log.
    Mod(String, crate::modlog::Entry),
    /// Something to tell in that channel's chat.
    Notice(String, String),
}

/// The running connection; dropping it ends the thread.
pub struct EventSub {
    tx: Sender<EsCmd>,
}

impl EventSub {
    pub fn start(emit: impl Fn(EsEvent) + Send + 'static) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || run(rx, emit));
        Self { tx }
    }

    pub fn send(&self, c: EsCmd) {
        let _ = self.tx.send(c);
    }
}

#[derive(Default)]
struct State {
    auth: Option<EsAuth>,
    /// login -> Twitch id
    watched: BTreeMap<String, String>,
    /// Channels where nothing could be subscribed (not a mod, or no permissions): not
    /// tried again until the sign-in changes.
    failed: HashSet<String>,
    /// Subscription ids per channel (to remove them when a chat closes).
    subs: HashMap<String, Vec<String>>,
}

impl State {
    fn wanted(&self) -> bool {
        self.auth.is_some() && self.watched.keys().any(|l| !self.failed.contains(l))
    }

    /// Applies a command; true when the sign-in changed.
    fn apply(&mut self, c: EsCmd) -> bool {
        match c {
            EsCmd::Auth(a) => {
                if a != self.auth {
                    self.auth = a;
                    self.failed.clear();
                    self.subs.clear();
                    return true;
                }
            }
            EsCmd::Watch(login, id) => {
                self.watched.insert(login, id);
            }
            EsCmd::Unwatch(login) => {
                self.watched.remove(&login);
                self.failed.remove(&login);
                self.subs.remove(&login);
            }
        }
        false
    }
}

fn run(rx: Receiver<EsCmd>, emit: impl Fn(EsEvent)) {
    let mut st = State::default();
    let mut backoff = 0u64;
    loop {
        while !st.wanted() {
            match rx.recv() {
                Ok(c) => {
                    st.apply(c);
                }
                Err(_) => return,
            }
        }
        if backoff > 0 {
            std::thread::sleep(Duration::from_secs(backoff));
        }
        match session(&mut st, &rx, &emit) {
            Ok(true) => return,
            Ok(false) => backoff = 0,
            Err(_) => backoff = (backoff * 2).clamp(2, 60),
        }
    }
}

fn set_timeout(s: &MaybeTlsStream<TcpStream>) {
    let t = Some(Duration::from_millis(200));
    match s {
        MaybeTlsStream::Plain(s) => {
            let _ = s.set_read_timeout(t);
        }
        MaybeTlsStream::Rustls(s) => {
            let _ = s.get_ref().set_read_timeout(t);
        }
        _ => {}
    }
}

/// One connection. Ok(true): the app is gone; Ok(false): connect again.
fn session(st: &mut State, rx: &Receiver<EsCmd>, emit: &impl Fn(EsEvent)) -> Result<bool, String> {
    let (mut ws, _) = tungstenite::connect(URL).map_err(|e| e.to_string())?;
    set_timeout(ws.get_ref());
    // the welcome carries the session id the subscriptions are made for
    let start = Instant::now();
    let (session_id, keepalive) = loop {
        if start.elapsed() > Duration::from_secs(15) {
            return Err("no welcome from Twitch".into());
        }
        match ws.read() {
            Ok(Message::Text(t)) => {
                let v: Value = serde_json::from_str(&t).unwrap_or(Value::Null);
                if v["metadata"]["message_type"] == "session_welcome" {
                    let s = &v["payload"]["session"];
                    break (
                        s["id"].as_str().unwrap_or_default().to_string(),
                        s["keepalive_timeout_seconds"].as_u64().unwrap_or(30),
                    );
                }
            }
            Ok(Message::Close(_)) => return Err("closed".into()),
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(e) => return Err(e.to_string()),
        }
    };
    st.subs.clear();
    let channels: Vec<(String, String)> = st
        .watched
        .iter()
        .filter(|(l, _)| !st.failed.contains(*l))
        .map(|(l, id)| (l.clone(), id.clone()))
        .collect();
    for (login, id) in channels {
        subscribe(st, &session_id, &login, &id, emit);
    }
    if st.subs.is_empty() {
        // nothing could be subscribed: Twitch would close it anyway
        let _ = ws.close(None);
        return Ok(false);
    }
    let mut last = Instant::now();
    loop {
        loop {
            match rx.try_recv() {
                Ok(EsCmd::Watch(login, id)) => {
                    let new = st.watched.get(&login) != Some(&id);
                    st.watched.insert(login.clone(), id.clone());
                    if new && !st.subs.contains_key(&login) {
                        subscribe(st, &session_id, &login, &id, emit);
                    }
                }
                Ok(EsCmd::Unwatch(login)) => {
                    st.watched.remove(&login);
                    st.failed.remove(&login);
                    if let (Some(ids), Some(a)) = (st.subs.remove(&login), st.auth.as_ref()) {
                        for id in ids {
                            let _ = helix(
                                a,
                                "DELETE",
                                &format!("eventsub/subscriptions?id={id}"),
                                None,
                            );
                        }
                    }
                }
                Ok(c) => {
                    if st.apply(c) {
                        let _ = ws.close(None);
                        return Ok(false);
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => return Ok(true),
            }
        }
        if st.subs.is_empty() {
            let _ = ws.close(None);
            return Ok(false);
        }
        // Twitch sends something at least every keepalive period
        if last.elapsed() > Duration::from_secs(keepalive + 10) {
            return Err("Twitch stopped answering".into());
        }
        match ws.read() {
            Ok(Message::Text(t)) => {
                last = Instant::now();
                let v: Value = serde_json::from_str(&t).unwrap_or(Value::Null);
                match v["metadata"]["message_type"].as_str().unwrap_or_default() {
                    "notification" => {
                        if let Some(e) = parse_notification(&v) {
                            emit(e);
                        }
                    }
                    // a fresh connection (and subscriptions) instead of moving this one
                    "session_reconnect" => {
                        let _ = ws.close(None);
                        return Ok(false);
                    }
                    _ => {}
                }
            }
            Ok(Message::Close(_)) => return Err("closed by Twitch".into()),
            Ok(_) => last = Instant::now(),
            Err(tungstenite::Error::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(e) => return Err(e.to_string()),
        }
    }
}

/// Subscribes a channel's kinds; tells the chat once when permissions are missing.
fn subscribe(st: &mut State, session_id: &str, login: &str, id: &str, emit: &impl Fn(EsEvent)) {
    let Some(auth) = st.auth.clone() else { return };
    let mut ids = Vec::new();
    let mut missing = false;
    let mut not_mod = false;
    for (kind, version, needs) in KINDS {
        if !auth.scopes.is_empty() && !needs.iter().all(|s| auth.scopes.iter().any(|h| h == s)) {
            missing = true;
            continue;
        }
        let body = serde_json::json!({
            "type": kind,
            "version": version,
            "condition": {"broadcaster_user_id": id, "moderator_user_id": auth.user_id},
            "transport": {"method": "websocket", "session_id": session_id},
        });
        match helix(&auth, "POST", "eventsub/subscriptions", Some(body)) {
            Ok((202, body)) => {
                let v: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
                ids.push(v["data"][0]["id"].as_str().unwrap_or_default().to_string());
            }
            // already there
            Ok((409, _)) => ids.push(String::new()),
            Ok((403, _)) => not_mod = true,
            _ => {}
        }
    }
    if missing {
        emit(EsEvent::Notice(
            login.to_string(),
            "AutoMod, unban requests and the mod log need new Twitch permissions: sign in to Twitch again (Settings > Accounts).".into(),
        ));
    }
    if ids.is_empty() {
        st.failed.insert(login.to_string());
        if not_mod && !missing {
            // the chat said moderator, Twitch disagrees: nothing to show
        }
    } else {
        st.subs.insert(login.to_string(), ids);
    }
}

fn helix(
    a: &EsAuth,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> Result<(u16, String), String> {
    let auth = crate::moderation::ModAuth {
        client_id: a.client_id.clone(),
        token: a.token.clone(),
        user_id: a.user_id.clone(),
    };
    crate::moderation::helix(&auth, method, path, body)
}

/// The clock time of a Twitch timestamp, in local time like chat lines ("HH:MM").
fn clock_of(ts: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(ts)
        .map(|t| t.with_timezone(&chrono::Local).format("%H:%M").to_string())
        .unwrap_or_else(|_| chrono::Local::now().format("%H:%M").to_string())
}

fn unix_of(ts: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(ts)
        .map(|t| t.timestamp())
        .unwrap_or_else(|_| chrono::Utc::now().timestamp())
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

/// A notification message -> what happened.
pub fn parse_notification(v: &Value) -> Option<EsEvent> {
    let kind = v["payload"]["subscription"]["type"]
        .as_str()
        .or_else(|| v["metadata"]["subscription_type"].as_str())?;
    let ev = &v["payload"]["event"];
    let channel = ev["broadcaster_user_login"].as_str()?.to_lowercase();
    let when = v["metadata"]["message_timestamp"]
        .as_str()
        .unwrap_or_default();
    Some(match kind {
        "automod.message.hold" => {
            let reason = match ev["reason"].as_str() {
                Some("blocked_term") => {
                    let terms: Vec<String> = ev["blocked_term"]["terms_found"]
                        .as_array()
                        .map(|t| {
                            t.iter()
                                .filter_map(|x| x["term"].as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default();
                    if terms.is_empty() {
                        "blocked term".to_string()
                    } else {
                        format!("blocked term: {}", terms.join(", "))
                    }
                }
                Some("blocked_link") => "blocked link".into(),
                _ => {
                    let cat = s(&ev["automod"]["category"]);
                    let level = ev["automod"]["level"].as_u64().unwrap_or(0);
                    if cat.is_empty() {
                        "AutoMod".into()
                    } else {
                        format!("{cat}, level {level}")
                    }
                }
            };
            EsEvent::Held(
                channel,
                crate::chat::HeldMessage {
                    time: clock_of(ev["held_at"].as_str().unwrap_or(when)),
                    msg_id: s(&ev["message_id"]),
                    user_id: s(&ev["user_id"]),
                    user: s(&ev["user_name"]),
                    text: s(&ev["message"]["text"]),
                    reason,
                    status: String::new(),
                },
            )
        }
        "automod.message.update" => EsEvent::HeldDone(
            channel,
            s(&ev["message_id"]),
            s(&ev["status"]).to_lowercase(),
            s(&ev["moderator_user_name"]),
        ),
        "channel.unban_request.create" => EsEvent::UnbanRequest(
            channel,
            crate::moderation::UnbanRequest {
                id: s(&ev["id"]),
                user_id: s(&ev["user_id"]),
                user_login: s(&ev["user_login"]),
                user_name: s(&ev["user_name"]),
                text: s(&ev["text"]),
                created_at: s(&ev["created_at"]),
                status: "pending".into(),
                ..Default::default()
            },
        ),
        "channel.unban_request.resolve" => EsEvent::UnbanResolved(
            channel,
            s(&ev["id"]),
            s(&ev["status"]),
            s(&ev["moderator_user_name"]),
        ),
        "channel.moderate" => {
            EsEvent::Mod(channel, crate::modlog::from_moderate(ev, unix_of(when))?)
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(kind: &str, event: Value) -> Value {
        serde_json::json!({
            "metadata": {"message_type": "notification", "message_timestamp": "2026-11-17T20:00:00Z", "subscription_type": kind},
            "payload": {"subscription": {"type": kind, "version": "2"}, "event": event}
        })
    }

    #[test]
    fn held_messages_and_their_outcome() {
        let hold = note(
            "automod.message.hold",
            serde_json::json!({
                "broadcaster_user_id": "1337", "broadcaster_user_login": "blah", "broadcaster_user_name": "blah",
                "user_id": "4242", "user_login": "baduser", "user_name": "baduser",
                "message_id": "bad-message-id",
                "message": {"text": "This is a bad message… pogchamp", "fragments": []},
                "reason": "automod",
                "automod": {"category": "aggressive", "level": 1, "boundaries": []},
                "blocked_term": null,
                "held_at": "2022-12-02T15:00:00.00Z"
            }),
        );
        match parse_notification(&hold) {
            Some(EsEvent::Held(ch, m)) => {
                assert_eq!(ch, "blah");
                assert_eq!(m.msg_id, "bad-message-id");
                assert_eq!(m.user_id, "4242");
                assert_eq!(m.user, "baduser");
                assert_eq!(m.reason, "aggressive, level 1");
                assert!(m.status.is_empty());
            }
            e => panic!("{e:?}"),
        }
        let update = note(
            "automod.message.update",
            serde_json::json!({
                "broadcaster_user_login": "blah", "message_id": "bad-message-id",
                "moderator_user_name": "the_mod", "status": "Approved"
            }),
        );
        assert_eq!(
            parse_notification(&update),
            Some(EsEvent::HeldDone(
                "blah".into(),
                "bad-message-id".into(),
                "approved".into(),
                "the_mod".into()
            ))
        );
    }

    #[test]
    fn unban_requests_arrive_and_get_resolved() {
        let create = note(
            "channel.unban_request.create",
            serde_json::json!({
                "id": "60", "broadcaster_user_id": "1337", "broadcaster_user_login": "cool_user",
                "broadcaster_user_name": "Cool_User", "user_id": "1339", "user_login": "not_cool_user",
                "user_name": "Not_Cool_User", "text": "unban me",
                "created_at": "2023-11-16T10:11:12.634234626Z"
            }),
        );
        match parse_notification(&create) {
            Some(EsEvent::UnbanRequest(ch, r)) => {
                assert_eq!(ch, "cool_user");
                assert_eq!(
                    (r.id.as_str(), r.user_login.as_str()),
                    ("60", "not_cool_user")
                );
                assert_eq!(r.text, "unban me");
                assert_eq!(r.status, "pending");
            }
            e => panic!("{e:?}"),
        }
        let resolve = note(
            "channel.unban_request.resolve",
            serde_json::json!({
                "id": "60", "broadcaster_user_login": "cool_user", "moderator_user_name": "Cool_User",
                "user_login": "not_cool_user", "resolution_text": "no", "status": "denied"
            }),
        );
        assert_eq!(
            parse_notification(&resolve),
            Some(EsEvent::UnbanResolved(
                "cool_user".into(),
                "60".into(),
                "denied".into(),
                "Cool_User".into()
            ))
        );
    }

    #[test]
    fn mod_actions_go_to_the_mod_log() {
        let warn = note(
            "channel.moderate",
            serde_json::json!({
                "broadcaster_user_id": "423374343", "broadcaster_user_login": "glowillig",
                "moderator_user_login": "lezzy", "moderator_user_name": "Lezzy",
                "action": "warn",
                "warn": {"user_id": "141981764", "user_login": "twitchdev", "user_name": "TwitchDev", "reason": "cut it out", "chat_rules_cited": null}
            }),
        );
        match parse_notification(&warn) {
            Some(EsEvent::Mod(ch, e)) => {
                assert_eq!(ch, "glowillig");
                assert_eq!(e.describe(), "Lezzy warned twitchdev: cut it out");
            }
            e => panic!("{e:?}"),
        }
        // anything else Twitch might send is ignored
        assert_eq!(
            parse_notification(&note(
                "channel.follow",
                serde_json::json!({"broadcaster_user_login": "x"})
            )),
            None
        );
    }
}
