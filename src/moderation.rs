//! Moderation in Twitch channels you moderate, as Chatterino's moderation buttons and
//! user timeout buttons: timeout, ban, unban and delete a message through Twitch's API
//! (Helix moderation/bans and moderation/chat). Needs the sign-in permissions
//! moderator:manage:banned_users and moderator:manage:chat_messages.

use serde_json::Value;
use std::time::Duration;

pub const HELIX: &str = "https://api.twitch.tv/helix/";

/// What to do to a chatter.
#[derive(Clone, Debug, PartialEq)]
pub enum ModKind {
    /// Seconds.
    Timeout(u64),
    Ban,
    Unban,
    /// Delete one message (its id).
    Delete(String),
    /// Allow or deny a message AutoMod held (its id).
    AutoMod {
        msg_id: String,
        allow: bool,
    },
    /// Pin a message (its id) for some seconds (30 to 1800), or None: until the stream ends.
    Pin {
        msg_id: String,
        secs: Option<u64>,
    },
    /// Unpin the pinned message (its id).
    Unpin(String),
}

/// One moderation action from a message.
#[derive(Clone, Debug)]
pub struct ModAction {
    pub user: String,
    pub user_id: String,
    pub kind: ModKind,
}

impl ModAction {
    pub fn describe(&self) -> String {
        match &self.kind {
            ModKind::Timeout(s) => format!("timed out {} for {}", self.user, duration_text(*s)),
            ModKind::Ban => format!("banned {}", self.user),
            ModKind::Unban => format!("unbanned {}", self.user),
            ModKind::Delete(_) => format!("deleted a message of {}", self.user),
            ModKind::AutoMod { allow: true, .. } => {
                format!("allowed the held message of {}", self.user)
            }
            ModKind::AutoMod { allow: false, .. } => {
                format!("denied the held message of {}", self.user)
            }
            ModKind::Pin { secs: Some(s), .. } => {
                format!(
                    "pinned a message of {} for {}",
                    self.user,
                    duration_text(*s)
                )
            }
            ModKind::Pin { secs: None, .. } => {
                format!("pinned a message of {} until the stream ends", self.user)
            }
            ModKind::Unpin(_) => "unpinned the pinned message".into(),
        }
    }
}

/// A channel's pinned message (Twitch's Get Pinned Chat Message).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pin {
    pub message_id: String,
    pub sender: String,
    pub text: String,
    pub pinned_by: String,
    /// When it ends (RFC 3339), empty: when the stream ends.
    pub ends_at: String,
}

/// The pinned message from Twitch's answer, if any.
pub fn parse_pin(body: &str) -> Option<Pin> {
    let v: Value = serde_json::from_str(body).ok()?;
    let p = &v["data"][0];
    let s = |x: &Value| x.as_str().unwrap_or_default().to_string();
    let id = p["message_id"].as_str()?;
    Some(Pin {
        message_id: id.to_string(),
        sender: s(&p["sender_user_name"]),
        text: s(&p["message"]["text"]),
        pinned_by: s(&p["pinned_by_user_name"]),
        ends_at: s(&p["ends_at"]),
    })
}

/// The channel's pinned message (moderators only).
pub fn get_pin(auth: &ModAuth, broadcaster_id: &str) -> Result<Option<Pin>, String> {
    let body = ok_or_explain(helix(
        auth,
        "GET",
        &format!(
            "chat/pins?broadcaster_id={broadcaster_id}&moderator_id={}",
            auth.user_id
        ),
        None,
    )?)?;
    Ok(parse_pin(&body))
}

/// An unban request (Twitch's Get Unban Requests, or EventSub).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UnbanRequest {
    pub id: String,
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub text: String,
    pub created_at: String,
    /// pending, approved, denied, acknowledged, canceled
    pub status: String,
    pub moderator: String,
    pub resolution: String,
}

pub fn parse_unban_requests(body: &str) -> Vec<UnbanRequest> {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let s = |x: &Value| x.as_str().unwrap_or_default().to_string();
    v["data"]
        .as_array()
        .map(|list| {
            list.iter()
                .map(|r| UnbanRequest {
                    id: s(&r["id"]),
                    user_id: s(&r["user_id"]),
                    user_login: s(&r["user_login"]),
                    user_name: s(&r["user_name"]),
                    text: s(&r["text"]),
                    created_at: s(&r["created_at"]),
                    status: s(&r["status"]),
                    moderator: s(&r["moderator_name"]),
                    resolution: s(&r["resolution_text"]),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A channel's pending unban requests (up to 100).
pub fn unban_requests(auth: &ModAuth, broadcaster_id: &str) -> Result<Vec<UnbanRequest>, String> {
    let body = ok_or_explain(helix(
        auth,
        "GET",
        &format!(
            "moderation/unban_requests?broadcaster_id={broadcaster_id}&moderator_id={}&status=pending&first=100",
            auth.user_id
        ),
        None,
    )?)?;
    Ok(parse_unban_requests(&body))
}

/// Approves or denies an unban request, with an optional note to the user.
pub fn resolve_unban(
    auth: &ModAuth,
    broadcaster_id: &str,
    request_id: &str,
    approve: bool,
    note: &str,
) -> Result<(), String> {
    let mut path = format!(
        "moderation/unban_requests?broadcaster_id={broadcaster_id}&moderator_id={}&unban_request_id={}&status={}",
        auth.user_id,
        urlencode(request_id),
        if approve { "approved" } else { "denied" }
    );
    let note: String = note.trim().chars().take(500).collect();
    if !note.is_empty() {
        path.push_str(&format!("&resolution_text={}", urlencode(&note)));
    }
    ok_or_explain(helix(auth, "PATCH", &path, None)?).map(|_| ())
}

/// The sign-in to act with.
#[derive(Clone)]
pub struct ModAuth {
    pub client_id: String,
    pub token: String,
    /// The moderator (the signed-in account).
    pub user_id: String,
}

/// "30s", "10m", "1h", "1d", "1w" -> seconds (Chatterino's timeout button units).
pub fn parse_duration(s: &str) -> Option<u64> {
    let s = s.trim().to_lowercase();
    let (num, unit) = s.split_at(s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len()));
    let n: u64 = num.parse().ok()?;
    let mult = match unit.trim() {
        "" | "s" => 1,
        "m" => 60,
        "h" => 3600,
        "d" => 86_400,
        "w" => 604_800,
        _ => return None,
    };
    let secs = n * mult;
    // Twitch allows 1 second to 2 weeks
    (1..=1_209_600).contains(&secs).then_some(secs)
}

pub fn duration_text(secs: u64) -> String {
    match secs {
        s if s % 604_800 == 0 => format!("{}w", s / 604_800),
        s if s % 86_400 == 0 => format!("{}d", s / 86_400),
        s if s % 3600 == 0 => format!("{}h", s / 3600),
        s if s % 60 == 0 => format!("{}m", s / 60),
        s => format!("{s}s"),
    }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .http_status_as_error(false)
        .build()
        .into()
}

fn explain(status: u16, body: &str) -> String {
    let msg = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("message").and_then(Value::as_str).map(String::from))
        .unwrap_or_default();
    match status {
        401 if msg.to_lowercase().contains("scope") => {
            "sign in to Twitch again (Settings > Accounts) to give Yaperino moderation rights"
                .into()
        }
        401 => "Twitch refused the sign-in".into(),
        403 => "you aren't a moderator in this channel".into(),
        _ if !msg.is_empty() => msg,
        _ => format!("Twitch answered {status}"),
    }
}

/// Runs a moderation action in a channel (its Twitch id).
pub fn run(auth: &ModAuth, broadcaster_id: &str, action: &ModAction) -> Result<(), String> {
    let a = agent();
    let q = format!(
        "broadcaster_id={broadcaster_id}&moderator_id={}",
        auth.user_id
    );
    let bearer = format!("Bearer {}", auth.token);
    let (status, body) = match &action.kind {
        ModKind::Timeout(_) | ModKind::Ban => {
            let mut data = serde_json::json!({ "user_id": action.user_id, "reason": "" });
            if let ModKind::Timeout(s) = action.kind {
                data["duration"] = serde_json::json!(s);
            }
            let mut r = a
                .post(&format!("{HELIX}moderation/bans?{q}"))
                .header("Authorization", &bearer)
                .header("Client-Id", &auth.client_id)
                .header("Content-Type", "application/json")
                .send(serde_json::json!({ "data": data }).to_string())
                .map_err(|e| e.to_string())?;
            (
                r.status().as_u16(),
                r.body_mut().read_to_string().unwrap_or_default(),
            )
        }
        ModKind::Unban => {
            let mut r = a
                .delete(&format!(
                    "{HELIX}moderation/bans?{q}&user_id={}",
                    action.user_id
                ))
                .header("Authorization", &bearer)
                .header("Client-Id", &auth.client_id)
                .call()
                .map_err(|e| e.to_string())?;
            (
                r.status().as_u16(),
                r.body_mut().read_to_string().unwrap_or_default(),
            )
        }
        ModKind::Delete(id) => {
            let mut r = a
                .delete(&format!("{HELIX}moderation/chat?{q}&message_id={id}"))
                .header("Authorization", &bearer)
                .header("Client-Id", &auth.client_id)
                .call()
                .map_err(|e| e.to_string())?;
            (
                r.status().as_u16(),
                r.body_mut().read_to_string().unwrap_or_default(),
            )
        }
        // the moderator's own id goes in user_id (Manage Held AutoMod Messages)
        ModKind::AutoMod { msg_id, allow } => helix(
            auth,
            "POST",
            "moderation/automod/message",
            Some(serde_json::json!({
                "user_id": auth.user_id,
                "msg_id": msg_id,
                "action": if *allow { "ALLOW" } else { "DENY" },
            })),
        )?,
        ModKind::Pin { msg_id, secs } => {
            let mut path = format!("chat/pins?{q}&message_id={}", urlencode(msg_id));
            if let Some(s) = secs {
                path.push_str(&format!("&duration_seconds={}", s.clamp(&30, &1800)));
            }
            helix(auth, "PUT", &path, None)?
        }
        ModKind::Unpin(msg_id) => helix(
            auth,
            "DELETE",
            &format!("chat/pins?{q}&message_id={}", urlencode(msg_id)),
            None,
        )?,
    };
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(explain(status, &body))
    }
}

// ------------------------------------------------------------- mod commands

/// Chatterino's / Moltorino's moderator and broadcaster commands, run through Twitch's API
/// (Twitch chat no longer runs "/timeout" and the like itself).
#[derive(Clone, Debug, PartialEq)]
pub enum ModCommand {
    Timeout {
        user: String,
        secs: u64,
        reason: String,
    },
    Ban {
        user: String,
        reason: String,
    },
    Unban(String),
    Warn {
        user: String,
        reason: String,
    },
    Clear,
    /// Chat modes: None = off.
    Slow(Option<u64>),
    Followers(Option<u64>),
    EmoteOnly(bool),
    SubOnly(bool),
    UniqueChat(bool),
    Shield(bool),
    Mod(String, bool),
    Vip(String, bool),
    Announce {
        color: String,
        text: String,
    },
    Shoutout(String),
    Raid(String),
    Unraid,
    Commercial(u64),
    Title(String),
    Game(String),
    Marker(String),
    Poll {
        title: String,
        choices: Vec<String>,
        secs: u64,
    },
    EndPoll {
        archive: bool,
    },
    Prediction {
        title: String,
        outcomes: Vec<String>,
        secs: u64,
    },
    LockPrediction,
    CancelPrediction,
    /// The winning outcome, counted from 1.
    CompletePrediction(usize),
    BlockTerm(String),
    UnblockTerm(String),
}

/// The text of /modhelp.
pub const HELP: &str = "Mod commands: /timeout <user> [10m] [reason], /ban <user> [reason], /unban <user>, /warn <user> <reason>, /clear, /slow [30] | /slowoff, /followers [10m] | /followersoff, /emoteonly | /emoteonlyoff, /subscribers | /subscribersoff, /uniquechat | /uniquechatoff, /shield | /shieldoff, /announce[blue|green|orange|purple] <text>, /shoutout <user>, /blockterm <text>, /unblockterm <text>, /modlogs [user], /unbanrequests (right-click a message to pin it). Broadcaster: /mod, /unmod, /vip, /unvip <user>, /raid <channel>, /unraid, /commercial [30], /settitle <title>, /setgame <game>, /marker [text], /poll <title> | <choice> | <choice> [| 60s], /endpoll, /archivepoll, /prediction <title> | <outcome> | <outcome> [| 2m], /lockprediction, /cancelprediction, /completeprediction <outcome number>.";

/// A typed command: None = not a mod command; Some(Err) = how to use it.
pub fn parse_command(text: &str) -> Option<Result<ModCommand, String>> {
    let text = text.trim();
    let (cmd, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    let cmd = cmd.to_lowercase();
    let rest = rest.trim();
    let mut words = rest.split_whitespace();
    let user = words
        .next()
        .map(|u| u.trim_start_matches('@').to_lowercase())
        .filter(|u| !u.is_empty());
    let after_user: String = rest
        .split_once(char::is_whitespace)
        .map(|(_, r)| r.trim().to_string())
        .unwrap_or_default();
    let usage = |u: &str| Some(Err(format!("Usage: {u}")));
    let need_user = |u: Option<String>, use_: &str, f: &dyn Fn(String) -> ModCommand| match u {
        Some(u) => Some(Ok(f(u))),
        None => Some(Err(format!("Usage: {use_}"))),
    };
    // "|"-separated parts, the last one a duration when it is one
    let parts = |default_secs: u64| -> (Vec<String>, u64) {
        let mut p: Vec<String> = rest
            .split('|')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let secs = p.last().and_then(|l| parse_duration(l)).inspect(|_| {
            p.pop();
        });
        (p, secs.unwrap_or(default_secs))
    };
    Some(Ok(match cmd.as_str() {
        "/timeout" | "/to" => {
            let Some(user) = user else {
                return usage("/timeout <user> [duration] [reason]");
            };
            let mut rest_words = after_user.splitn(2, char::is_whitespace);
            let first = rest_words.next().unwrap_or("");
            let (secs, reason) = match parse_duration(first) {
                Some(s) => (s, rest_words.next().unwrap_or("").trim().to_string()),
                None if first.is_empty() => (600, String::new()),
                None => (600, after_user.clone()),
            };
            ModCommand::Timeout { user, secs, reason }
        }
        "/ban" => {
            return need_user(user, "/ban <user> [reason]", &|u| ModCommand::Ban {
                user: u,
                reason: after_user.clone(),
            })
        }
        "/unban" | "/untimeout" => {
            return need_user(user, "/unban <user>", &|u| ModCommand::Unban(u))
        }
        "/warn" => {
            if after_user.is_empty() {
                return usage("/warn <user> <reason>");
            }
            return need_user(user, "/warn <user> <reason>", &|u| ModCommand::Warn {
                user: u,
                reason: after_user.clone(),
            });
        }
        "/clear" => ModCommand::Clear,
        "/slow" => ModCommand::Slow(Some(if rest.is_empty() {
            30
        } else {
            match rest.parse::<u64>().ok().or_else(|| parse_duration(rest)) {
                Some(s) if (3..=120).contains(&s) => s,
                _ => return usage("/slow [3-120 seconds]"),
            }
        })),
        "/slowoff" => ModCommand::Slow(None),
        "/followers" => ModCommand::Followers(Some(if rest.is_empty() {
            0
        } else {
            match parse_duration(rest) {
                Some(s) if s / 60 <= 129_600 => s / 60,
                _ => return usage("/followers [duration, e.g. 10m, up to 3 months]"),
            }
        })),
        "/followersoff" => ModCommand::Followers(None),
        "/emoteonly" => ModCommand::EmoteOnly(true),
        "/emoteonlyoff" => ModCommand::EmoteOnly(false),
        "/subscribers" => ModCommand::SubOnly(true),
        "/subscribersoff" => ModCommand::SubOnly(false),
        "/uniquechat" | "/r9kbeta" => ModCommand::UniqueChat(true),
        "/uniquechatoff" | "/r9kbetaoff" => ModCommand::UniqueChat(false),
        "/shield" => ModCommand::Shield(true),
        "/shieldoff" => ModCommand::Shield(false),
        "/mod" => return need_user(user, "/mod <user>", &|u| ModCommand::Mod(u, true)),
        "/unmod" => return need_user(user, "/unmod <user>", &|u| ModCommand::Mod(u, false)),
        "/vip" => return need_user(user, "/vip <user>", &|u| ModCommand::Vip(u, true)),
        "/unvip" => return need_user(user, "/unvip <user>", &|u| ModCommand::Vip(u, false)),
        c if c.starts_with("/announce") => {
            let color = match &c["/announce".len()..] {
                "" => "primary",
                "blue" => "blue",
                "green" => "green",
                "orange" => "orange",
                "purple" => "purple",
                _ => return None,
            };
            if rest.is_empty() {
                return usage("/announce <text>");
            }
            ModCommand::Announce {
                color: color.into(),
                text: rest.into(),
            }
        }
        "/shoutout" | "/so" => {
            return need_user(user, "/shoutout <user>", &|u| ModCommand::Shoutout(u))
        }
        "/raid" => return need_user(user, "/raid <channel>", &|u| ModCommand::Raid(u)),
        "/unraid" => ModCommand::Unraid,
        "/commercial" => ModCommand::Commercial(match rest {
            "" => 30,
            r => match r.trim_end_matches('s').parse::<u64>() {
                Ok(n) if (1..=180).contains(&n) => n,
                _ => return usage("/commercial [length in seconds, up to 180]"),
            },
        }),
        "/settitle" if !rest.is_empty() => ModCommand::Title(rest.into()),
        "/settitle" => return usage("/settitle <title>"),
        "/setgame" if !rest.is_empty() => ModCommand::Game(rest.into()),
        "/setgame" => return usage("/setgame <game>"),
        "/marker" => ModCommand::Marker(rest.into()),
        "/poll" => {
            let (p, secs) = parts(60);
            if p.len() < 3 || p.len() > 6 || !(15..=1800).contains(&secs) {
                return usage("/poll <title> | <choice> | <choice> (2-5 choices) [| 15s-30m]");
            }
            ModCommand::Poll {
                title: p[0].clone(),
                choices: p[1..].to_vec(),
                secs,
            }
        }
        "/endpoll" => ModCommand::EndPoll { archive: false },
        "/archivepoll" | "/deletepoll" => ModCommand::EndPoll { archive: true },
        "/prediction" => {
            let (p, secs) = parts(120);
            if p.len() < 3 || p.len() > 11 || !(30..=1800).contains(&secs) {
                return usage(
                    "/prediction <title> | <outcome> | <outcome> (2-10 outcomes) [| 30s-30m]",
                );
            }
            ModCommand::Prediction {
                title: p[0].clone(),
                outcomes: p[1..].to_vec(),
                secs,
            }
        }
        "/lockprediction" | "/endprediction" => ModCommand::LockPrediction,
        "/cancelprediction" => ModCommand::CancelPrediction,
        "/completeprediction" => match rest.parse::<usize>() {
            Ok(n) if n >= 1 => ModCommand::CompletePrediction(n),
            _ => return usage("/completeprediction <winning outcome number, 1 = first>"),
        },
        "/blockterm" if !rest.is_empty() => ModCommand::BlockTerm(rest.into()),
        "/blockterm" => return usage("/blockterm <text>"),
        "/unblockterm" if !rest.is_empty() => ModCommand::UnblockTerm(rest.into()),
        "/unblockterm" => return usage("/unblockterm <text>"),
        _ => return None,
    }))
}

/// The sign-in permission a command needs.
pub fn required_scope(c: &ModCommand) -> &'static str {
    use ModCommand::*;
    match c {
        Timeout { .. } | Ban { .. } | Unban(_) => "moderator:manage:banned_users",
        Warn { .. } => "moderator:manage:warnings",
        Clear => "moderator:manage:chat_messages",
        Slow(_) | Followers(_) | EmoteOnly(_) | SubOnly(_) | UniqueChat(_) => {
            "moderator:manage:chat_settings"
        }
        Shield(_) => "moderator:manage:shield_mode",
        Mod(..) => "channel:manage:moderators",
        Vip(..) => "channel:manage:vips",
        Announce { .. } => "moderator:manage:announcements",
        Shoutout(_) => "moderator:manage:shoutouts",
        Raid(_) | Unraid => "channel:manage:raids",
        Commercial(_) => "channel:edit:commercial",
        Title(_) | Game(_) | Marker(_) => "channel:manage:broadcast",
        Poll { .. } | EndPoll { .. } => "channel:manage:polls",
        Prediction { .. } | LockPrediction | CancelPrediction | CompletePrediction(_) => {
            "channel:manage:predictions"
        }
        BlockTerm(_) | UnblockTerm(_) => "moderator:manage:blocked_terms",
    }
}

/// One Helix call: (status, body).
pub(crate) fn helix(
    auth: &ModAuth,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> Result<(u16, String), String> {
    let a = agent();
    let url = format!("{HELIX}{path}");
    let bearer = format!("Bearer {}", auth.token);
    let json = body.map(|b| b.to_string());
    let mut r = match (method, json) {
        ("GET", _) => a
            .get(&url)
            .header("Authorization", &bearer)
            .header("Client-Id", &auth.client_id)
            .call(),
        ("DELETE", _) => a
            .delete(&url)
            .header("Authorization", &bearer)
            .header("Client-Id", &auth.client_id)
            .call(),
        (m, j) => {
            let j = j.unwrap_or_else(|| "{}".into());
            let req = match m {
                "POST" => a.post(&url),
                "PATCH" => a.patch(&url),
                _ => a.put(&url),
            };
            req.header("Authorization", &bearer)
                .header("Client-Id", &auth.client_id)
                .header("Content-Type", "application/json")
                .send(j)
        }
    }
    .map_err(|e| e.to_string())?;
    Ok((
        r.status().as_u16(),
        r.body_mut().read_to_string().unwrap_or_default(),
    ))
}

fn ok_or_explain((status, body): (u16, String)) -> Result<String, String> {
    if (200..300).contains(&status) {
        Ok(body)
    } else {
        Err(explain(status, &body))
    }
}

/// A Twitch user's id from their login name.
fn user_id(auth: &ModAuth, login: &str) -> Result<String, String> {
    let body = ok_or_explain(helix(
        auth,
        "GET",
        &format!("users?login={}", urlencode(login)),
        None,
    )?)?;
    let v: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    v["data"][0]["id"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| format!("no Twitch user called {login}"))
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Runs a mod command in a channel (its Twitch id); Ok is what to show in the chat.
pub fn run_command(auth: &ModAuth, b: &str, c: &ModCommand) -> Result<String, String> {
    use ModCommand::*;
    let me = &auth.user_id;
    let bm = format!("broadcaster_id={b}&moderator_id={me}");
    let settings = |body: Value, done: &str| -> Result<String, String> {
        ok_or_explain(helix(
            auth,
            "PATCH",
            &format!("chat/settings?{bm}"),
            Some(body),
        )?)
        .map(|_| done.to_string())
    };
    match c {
        Timeout { user, secs, reason } => {
            let id = user_id(auth, user)?;
            ok_or_explain(helix(
                auth,
                "POST",
                &format!("moderation/bans?{bm}"),
                Some(
                    serde_json::json!({ "data": { "user_id": id, "duration": secs, "reason": reason } }),
                ),
            )?)?;
            Ok(format!(
                "You timed out {user} for {}.",
                duration_text(*secs)
            ))
        }
        Ban { user, reason } => {
            let id = user_id(auth, user)?;
            ok_or_explain(helix(
                auth,
                "POST",
                &format!("moderation/bans?{bm}"),
                Some(serde_json::json!({ "data": { "user_id": id, "reason": reason } })),
            )?)?;
            Ok(format!("You banned {user}."))
        }
        Unban(user) => {
            let id = user_id(auth, user)?;
            ok_or_explain(helix(
                auth,
                "DELETE",
                &format!("moderation/bans?{bm}&user_id={id}"),
                None,
            )?)?;
            Ok(format!("You unbanned {user}."))
        }
        Warn { user, reason } => {
            let id = user_id(auth, user)?;
            ok_or_explain(helix(
                auth,
                "POST",
                &format!("moderation/warnings?{bm}"),
                Some(serde_json::json!({ "data": { "user_id": id, "reason": reason } })),
            )?)?;
            Ok(format!("You warned {user}."))
        }
        Clear => ok_or_explain(helix(
            auth,
            "DELETE",
            &format!("moderation/chat?{bm}"),
            None,
        )?)
        .map(|_| "You cleared the chat.".into()),
        Slow(Some(s)) => settings(
            serde_json::json!({ "slow_mode": true, "slow_mode_wait_time": s }),
            &format!("Slow mode on ({s}s)."),
        ),
        Slow(None) => settings(serde_json::json!({ "slow_mode": false }), "Slow mode off."),
        Followers(Some(m)) => settings(
            serde_json::json!({ "follower_mode": true, "follower_mode_duration": m }),
            &format!("Followers-only mode on ({m} minutes)."),
        ),
        Followers(None) => settings(
            serde_json::json!({ "follower_mode": false }),
            "Followers-only mode off.",
        ),
        EmoteOnly(on) => settings(
            serde_json::json!({ "emote_mode": on }),
            if *on {
                "Emote-only mode on."
            } else {
                "Emote-only mode off."
            },
        ),
        SubOnly(on) => settings(
            serde_json::json!({ "subscriber_mode": on }),
            if *on {
                "Subscribers-only mode on."
            } else {
                "Subscribers-only mode off."
            },
        ),
        UniqueChat(on) => settings(
            serde_json::json!({ "unique_chat_mode": on }),
            if *on {
                "Unique chat on."
            } else {
                "Unique chat off."
            },
        ),
        Shield(on) => ok_or_explain(helix(
            auth,
            "PUT",
            &format!("moderation/shield_mode?{bm}"),
            Some(serde_json::json!({ "is_active": on })),
        )?)
        .map(|_| {
            if *on {
                "Shield mode on.".into()
            } else {
                "Shield mode off.".into()
            }
        }),
        Mod(user, on) => {
            let id = user_id(auth, user)?;
            ok_or_explain(helix(
                auth,
                if *on { "POST" } else { "DELETE" },
                &format!("moderation/moderators?broadcaster_id={b}&user_id={id}"),
                None,
            )?)?;
            Ok(if *on {
                format!("You made {user} a moderator.")
            } else {
                format!("You removed {user} as a moderator.")
            })
        }
        Vip(user, on) => {
            let id = user_id(auth, user)?;
            ok_or_explain(helix(
                auth,
                if *on { "POST" } else { "DELETE" },
                &format!("channels/vips?broadcaster_id={b}&user_id={id}"),
                None,
            )?)?;
            Ok(if *on {
                format!("You made {user} a VIP.")
            } else {
                format!("You removed {user} as a VIP.")
            })
        }
        Announce { color, text } => {
            ok_or_explain(helix(
                auth,
                "POST",
                &format!("chat/announcements?{bm}"),
                Some(serde_json::json!({ "message": text, "color": color })),
            )?)?;
            Ok("Announcement sent.".into())
        }
        Shoutout(user) => {
            let id = user_id(auth, user)?;
            ok_or_explain(helix(auth, "POST", &format!("chat/shoutouts?from_broadcaster_id={b}&to_broadcaster_id={id}&moderator_id={me}"), None)?)?;
            Ok(format!("You gave {user} a shoutout."))
        }
        Raid(user) => {
            let id = user_id(auth, user)?;
            ok_or_explain(helix(
                auth,
                "POST",
                &format!("raids?from_broadcaster_id={b}&to_broadcaster_id={id}"),
                None,
            )?)?;
            Ok(format!(
                "Raiding {user}: Twitch starts the raid in about 90 seconds (/unraid stops it)."
            ))
        }
        Unraid => ok_or_explain(helix(
            auth,
            "DELETE",
            &format!("raids?broadcaster_id={b}"),
            None,
        )?)
        .map(|_| "Raid cancelled.".into()),
        Commercial(s) => {
            ok_or_explain(helix(
                auth,
                "POST",
                "channels/commercial",
                Some(serde_json::json!({ "broadcaster_id": b, "length": s })),
            )?)?;
            Ok(format!("A {s}-second ad break started."))
        }
        Title(t) => ok_or_explain(helix(
            auth,
            "PATCH",
            &format!("channels?broadcaster_id={b}"),
            Some(serde_json::json!({ "title": t })),
        )?)
        .map(|_| format!("Title set to: {t}")),
        Game(g) => {
            let body = ok_or_explain(helix(
                auth,
                "GET",
                &format!("search/categories?query={}&first=5", urlencode(g)),
                None,
            )?)?;
            let v: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            let list = v["data"].as_array().cloned().unwrap_or_default();
            // the exact name first, else Twitch's best match
            let pick = list
                .iter()
                .find(|c| {
                    c["name"]
                        .as_str()
                        .is_some_and(|n| n.eq_ignore_ascii_case(g))
                })
                .or(list.first())
                .ok_or_else(|| format!("no game called {g}"))?;
            let (id, name) = (
                pick["id"].as_str().unwrap_or_default(),
                pick["name"].as_str().unwrap_or(g),
            );
            ok_or_explain(helix(
                auth,
                "PATCH",
                &format!("channels?broadcaster_id={b}"),
                Some(serde_json::json!({ "game_id": id })),
            )?)?;
            Ok(format!("Category set to {name}."))
        }
        Marker(t) => ok_or_explain(helix(
            auth,
            "POST",
            "streams/markers",
            Some(serde_json::json!({ "user_id": b, "description": t })),
        )?)
        .map(|_| "Stream marker added.".into()),
        Poll {
            title,
            choices,
            secs,
        } => {
            let choices: Vec<Value> = choices
                .iter()
                .map(|c| serde_json::json!({ "title": c }))
                .collect();
            ok_or_explain(helix(
                auth,
                "POST",
                "polls",
                Some(
                    serde_json::json!({ "broadcaster_id": b, "title": title, "choices": choices, "duration": secs }),
                ),
            )?)?;
            Ok(format!("Poll started: {title} ({}).", duration_text(*secs)))
        }
        EndPoll { archive } => {
            let body = ok_or_explain(helix(
                auth,
                "GET",
                &format!("polls?broadcaster_id={b}&first=1"),
                None,
            )?)?;
            let v: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            let p = &v["data"][0];
            if p["status"].as_str() != Some("ACTIVE") && !archive {
                return Err("there's no running poll".into());
            }
            let id = p["id"].as_str().ok_or("there's no poll")?;
            let status = if *archive { "ARCHIVED" } else { "TERMINATED" };
            ok_or_explain(helix(
                auth,
                "PATCH",
                "polls",
                Some(serde_json::json!({ "broadcaster_id": b, "id": id, "status": status })),
            )?)?;
            Ok(if *archive {
                "Poll removed.".into()
            } else {
                "Poll ended.".into()
            })
        }
        Prediction {
            title,
            outcomes,
            secs,
        } => {
            let outcomes: Vec<Value> = outcomes
                .iter()
                .map(|o| serde_json::json!({ "title": o }))
                .collect();
            ok_or_explain(helix(
                auth,
                "POST",
                "predictions",
                Some(
                    serde_json::json!({ "broadcaster_id": b, "title": title, "outcomes": outcomes, "prediction_window": secs }),
                ),
            )?)?;
            Ok(format!(
                "Prediction started: {title} (open for {}).",
                duration_text(*secs)
            ))
        }
        LockPrediction | CancelPrediction | CompletePrediction(_) => {
            let body = ok_or_explain(helix(
                auth,
                "GET",
                &format!("predictions?broadcaster_id={b}&first=1"),
                None,
            )?)?;
            let v: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            let p = &v["data"][0];
            if !matches!(p["status"].as_str(), Some("ACTIVE") | Some("LOCKED")) {
                return Err("there's no open prediction".into());
            }
            let id = p["id"].as_str().unwrap_or_default();
            let mut body = serde_json::json!({ "broadcaster_id": b, "id": id });
            let done = match c {
                LockPrediction => {
                    body["status"] = "LOCKED".into();
                    "Prediction locked.".to_string()
                }
                CancelPrediction => {
                    body["status"] = "CANCELED".into();
                    "Prediction cancelled; points go back.".to_string()
                }
                CompletePrediction(n) => {
                    let o = p["outcomes"]
                        .get(n - 1)
                        .ok_or_else(|| format!("the prediction has no outcome {n}"))?;
                    body["status"] = "RESOLVED".into();
                    body["winning_outcome_id"] = o["id"].clone();
                    format!(
                        "Prediction resolved: {} won.",
                        o["title"].as_str().unwrap_or("?")
                    )
                }
                _ => unreachable!(),
            };
            ok_or_explain(helix(auth, "PATCH", "predictions", Some(body))?)?;
            Ok(done)
        }
        BlockTerm(t) => ok_or_explain(helix(
            auth,
            "POST",
            &format!("moderation/blocked_terms?{bm}"),
            Some(serde_json::json!({ "text": t })),
        )?)
        .map(|_| format!("Blocked term added: {t}")),
        UnblockTerm(t) => {
            // find its id among the blocked terms (pages of 100)
            let mut cursor = String::new();
            for _ in 0..20 {
                let after = if cursor.is_empty() {
                    String::new()
                } else {
                    format!("&after={cursor}")
                };
                let body = ok_or_explain(helix(
                    auth,
                    "GET",
                    &format!("moderation/blocked_terms?{bm}&first=100{after}"),
                    None,
                )?)?;
                let v: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
                if let Some(found) = v["data"].as_array().and_then(|l| {
                    l.iter().find(|x| {
                        x["text"]
                            .as_str()
                            .is_some_and(|x| x.eq_ignore_ascii_case(t))
                    })
                }) {
                    let id = found["id"].as_str().unwrap_or_default();
                    ok_or_explain(helix(
                        auth,
                        "DELETE",
                        &format!("moderation/blocked_terms?{bm}&id={id}"),
                        None,
                    )?)?;
                    return Ok(format!("Blocked term removed: {t}"));
                }
                match v["pagination"]["cursor"].as_str() {
                    Some(c) if !c.is_empty() => cursor = c.to_string(),
                    _ => break,
                }
            }
            Err(format!("{t} isn't a blocked term"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod_commands_parse() {
        let p = |t: &str| parse_command(t).map(|r| r.ok());
        assert_eq!(
            p("/timeout @Bob 5m spam"),
            Some(Some(ModCommand::Timeout {
                user: "bob".into(),
                secs: 300,
                reason: "spam".into()
            }))
        );
        assert_eq!(
            p("/timeout bob being rude"),
            Some(Some(ModCommand::Timeout {
                user: "bob".into(),
                secs: 600,
                reason: "being rude".into()
            }))
        );
        assert_eq!(
            p("/ban bob"),
            Some(Some(ModCommand::Ban {
                user: "bob".into(),
                reason: String::new()
            }))
        );
        assert!(matches!(parse_command("/ban"), Some(Err(_))));
        assert_eq!(p("/slow"), Some(Some(ModCommand::Slow(Some(30)))));
        assert_eq!(
            p("/followers 10m"),
            Some(Some(ModCommand::Followers(Some(10))))
        );
        assert_eq!(
            p("/announceblue hi all"),
            Some(Some(ModCommand::Announce {
                color: "blue".into(),
                text: "hi all".into()
            }))
        );
        assert_eq!(p("/announcement x"), None);
        assert_eq!(
            p("/poll Best? | a | b | 2m"),
            Some(Some(ModCommand::Poll {
                title: "Best?".into(),
                choices: vec!["a".into(), "b".into()],
                secs: 120
            }))
        );
        assert!(matches!(parse_command("/poll only | one"), Some(Err(_))));
        assert_eq!(
            p("/completeprediction 2"),
            Some(Some(ModCommand::CompletePrediction(2)))
        );
        assert_eq!(p("/me waves"), None);
        assert_eq!(p("hello"), None);
        assert_eq!(
            required_scope(&ModCommand::Clear),
            "moderator:manage:chat_messages"
        );
        assert_eq!(urlencode("Just Chatting"), "Just%20Chatting");
    }

    #[test]
    fn durations() {
        assert_eq!(parse_duration("30s"), Some(30));
        assert_eq!(parse_duration("10m"), Some(600));
        assert_eq!(parse_duration("1h"), Some(3600));
        assert_eq!(parse_duration("1d"), Some(86_400));
        assert_eq!(parse_duration("2w"), Some(1_209_600));
        assert_eq!(parse_duration("3w"), None);
        assert_eq!(parse_duration("x"), None);
        assert_eq!(duration_text(600), "10m");
        assert_eq!(duration_text(90), "90s");
    }

    #[test]
    fn pinned_message_and_unban_requests_are_read() {
        // shapes from Twitch's Get Pinned Chat Message / Get Unban Requests examples
        let pins = r#"{"data":[{"message_id":"abc-def-123-456","broadcaster_id":"197886470",
            "sender_user_id":"1","sender_user_login":"viewer","sender_user_name":"Viewer",
            "pinned_by_user_id":"2","pinned_by_user_login":"mod","pinned_by_user_name":"Mod",
            "message":{"text":"Welcome! bleedPurple Type !rules","fragments":[]},
            "starts_at":"2026-05-06T12:30:00Z","ends_at":null,"updated_at":"2026-05-06T12:30:00Z"}]}"#;
        assert_eq!(
            parse_pin(pins),
            Some(Pin {
                message_id: "abc-def-123-456".into(),
                sender: "Viewer".into(),
                text: "Welcome! bleedPurple Type !rules".into(),
                pinned_by: "Mod".into(),
                ends_at: String::new(),
            })
        );
        assert_eq!(parse_pin(r#"{"data":[]}"#), None);
        let unbans = r#"{"data":[{"id":"92af127c-7326-4483-a52b-b0da0be61c01",
            "broadcaster_name":"Simon","broadcaster_login":"simon","broadcaster_id":"274637212",
            "moderator_id":null,"moderator_login":null,"moderator_name":null,
            "user_id":"424596340","user_login":"quotrok","user_name":"quotrok",
            "text":"Please unban me from the channel?","status":"pending",
            "created_at":"2022-08-07T02:07:55Z","resolved_at":null,"resolution_text":null}],
            "pagination":{"cursor":"x"}}"#;
        let list = parse_unban_requests(unbans);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "92af127c-7326-4483-a52b-b0da0be61c01");
        assert_eq!(list[0].user_name, "quotrok");
        assert_eq!(list[0].text, "Please unban me from the channel?");
        assert_eq!(
            (list[0].status.as_str(), list[0].moderator.as_str()),
            ("pending", "")
        );
        // what clicking Allow / Pin says in the chat
        let m = |kind| ModAction {
            user: "Viewer".into(),
            user_id: "1".into(),
            kind,
        };
        assert_eq!(
            m(ModKind::AutoMod {
                msg_id: "x".into(),
                allow: true
            })
            .describe(),
            "allowed the held message of Viewer"
        );
        assert_eq!(
            m(ModKind::Pin {
                msg_id: "x".into(),
                secs: Some(300)
            })
            .describe(),
            "pinned a message of Viewer for 5m"
        );
    }
}
