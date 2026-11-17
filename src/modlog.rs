//! The mod log (Moltorino's /modlogs, built from official data): every moderation action
//! in the channels the account moderates, as Twitch's channel.moderate v2 reports them
//! while Yaperino runs. Twitch has no official way to read older actions, so the log starts
//! when Yaperino first watches a channel.
//!
//! Kept per channel in <Yaperino's folder>/ModLogs/<channel>.jsonl (one action per line,
//! the newest MAX_KEPT).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

/// How many actions are kept per channel.
const MAX_KEPT: usize = 5000;

/// One moderation action.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Entry {
    /// When (Unix seconds).
    pub time: i64,
    pub moderator: String,
    /// Twitch's action name (ban, timeout, delete, slow, ...).
    pub action: String,
    /// Who it was done to (login), if anyone.
    pub target: String,
    /// The reason, the deleted message, the duration, the terms ...
    pub detail: String,
}

impl Entry {
    /// "Mod timed out someone for 10m: reason".
    pub fn describe(&self) -> String {
        let (base, shared) = match self.action.strip_prefix("shared_chat_") {
            Some(a) => (a, true),
            None => (self.action.as_str(), false),
        };
        let verb = match base {
            "ban" => "banned",
            "unban" => "unbanned",
            "timeout" => "timed out",
            "untimeout" => "removed the timeout of",
            "delete" => "deleted a message of",
            "warn" => "warned",
            "clear" => "cleared the chat",
            "emoteonly" => "turned on emote-only mode",
            "emoteonlyoff" => "turned off emote-only mode",
            "followers" => "turned on followers-only mode",
            "followersoff" => "turned off followers-only mode",
            "uniquechat" => "turned on unique-chat mode",
            "uniquechatoff" => "turned off unique-chat mode",
            "slow" => "turned on slow mode",
            "slowoff" => "turned off slow mode",
            "subscribers" => "turned on subscribers-only mode",
            "subscribersoff" => "turned off subscribers-only mode",
            "raid" => "raided",
            "unraid" => "cancelled the raid",
            "vip" => "made a VIP:",
            "unvip" => "removed the VIP of",
            "mod" => "modded",
            "unmod" => "unmodded",
            "add_blocked_term" => "blocked a term",
            "add_permitted_term" => "permitted a term",
            "remove_blocked_term" => "removed a blocked term",
            "remove_permitted_term" => "removed a permitted term",
            "approve_unban_request" => "approved the unban request of",
            "deny_unban_request" => "denied the unban request of",
            other => other,
        };
        let mut s = format!("{} {verb}", self.moderator);
        if !self.target.is_empty() {
            s.push(' ');
            s.push_str(&self.target);
        }
        if shared {
            s.push_str(" (shared chat)");
        }
        if !self.detail.is_empty() {
            let joiner = if base == "timeout" { " " } else { ": " };
            s.push_str(joiner);
            s.push_str(&self.detail);
        }
        s
    }
}

/// A channel.moderate v2 event -> an entry (`time` from the notification).
pub fn from_moderate(ev: &Value, time: i64) -> Option<Entry> {
    let action = ev["action"].as_str()?.to_string();
    // the action's details are in a field named after it, except these two groups
    let obj = if action.ends_with("_term") {
        &ev["automod_terms"]
    } else if action.ends_with("_unban_request") {
        &ev["unban_request"]
    } else {
        &ev[action.as_str()]
    };
    let text = |k: &str| obj[k].as_str().unwrap_or_default().to_string();
    let base = action.trim_start_matches("shared_chat_");
    let detail = match base {
        "ban" | "warn" => text("reason"),
        "timeout" => {
            let until = obj["expires_at"]
                .as_str()
                .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                .map(|t| t.timestamp() - time);
            let mut d = until
                .filter(|s| *s > 0)
                .map(|s| format!("for {}", crate::moderation::duration_text(round(s as u64))))
                .unwrap_or_default();
            let reason = text("reason");
            if !reason.is_empty() {
                d = if d.is_empty() {
                    reason
                } else {
                    format!("{d}: {reason}")
                };
            }
            d
        }
        "delete" => text("message_body"),
        "slow" => obj["wait_time_seconds"]
            .as_u64()
            .map(|s| format!("{s}s"))
            .unwrap_or_default(),
        "followers" => obj["follow_duration_minutes"]
            .as_u64()
            .map(|m| format!("{m} minutes"))
            .unwrap_or_default(),
        "raid" => obj["viewer_count"]
            .as_u64()
            .map(|n| format!("{n} viewers"))
            .unwrap_or_default(),
        b if b.ends_with("_term") => obj["terms"]
            .as_array()
            .map(|t| {
                t.iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default(),
        b if b.ends_with("_unban_request") => text("moderator_message"),
        _ => String::new(),
    };
    Some(Entry {
        time,
        moderator: ev["moderator_user_name"]
            .as_str()
            .or(ev["moderator_user_login"].as_str())
            .unwrap_or_default()
            .to_string(),
        target: text("user_login"),
        action,
        detail,
    })
}

/// Timeouts end a moment after they're given: 599s reads as 10m.
fn round(secs: u64) -> u64 {
    for unit in [604_800, 86_400, 3600, 60] {
        let r = (secs + unit / 2) / unit * unit;
        if r > 0 && secs.abs_diff(r) <= 2 {
            return r;
        }
    }
    secs
}

fn dir() -> PathBuf {
    crate::paths::app_dir_or_here().join("ModLogs")
}

fn file(channel: &str) -> PathBuf {
    let safe: String = channel
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    dir().join(format!("{safe}.jsonl"))
}

/// A channel's saved actions, oldest first.
pub fn load(channel: &str) -> Vec<Entry> {
    std::fs::read_to_string(file(channel))
        .map(|t| parse_lines(&t))
        .unwrap_or_default()
}

fn parse_lines(t: &str) -> Vec<Entry> {
    t.lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// Adds an action to a channel's file; `all` is the channel's list with it (to rewrite the
/// file when it's grown past MAX_KEPT).
pub fn append(channel: &str, e: &Entry, all: &[Entry]) {
    use std::io::Write;
    let _ = std::fs::create_dir_all(dir());
    let path = file(channel);
    if all.len() > MAX_KEPT + MAX_KEPT / 10 {
        let keep = &all[all.len() - MAX_KEPT..];
        let text: String = keep
            .iter()
            .filter_map(|e| serde_json::to_string(e).ok())
            .map(|l| l + "\n")
            .collect();
        let _ = std::fs::write(&path, text);
        return;
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        if let Ok(l) = serde_json::to_string(e) {
            let _ = writeln!(f, "{l}");
        }
    }
}

/// Keeps a channel's in-memory list to the last MAX_KEPT.
pub fn trim(all: &mut Vec<Entry>) {
    if all.len() > MAX_KEPT {
        all.drain(..all.len() - MAX_KEPT);
    }
}

/// Sample actions (screenshot mode).
pub fn samples() -> Vec<Entry> {
    let now = chrono::Utc::now().timestamp();
    let e = |ago: i64, m: &str, a: &str, t: &str, d: &str| Entry {
        time: now - ago,
        moderator: m.into(),
        action: a.into(),
        target: t.into(),
        detail: d.into(),
    };
    vec![
        e(3600, "ModOne", "slow", "", "30s"),
        e(2400, "ModTwo", "timeout", "spammer42", "for 10m: spam"),
        e(
            1800,
            "ModOne",
            "delete",
            "AnotherViewer",
            "buy followers at ...",
        ),
        e(900, "ModTwo", "warn", "AnotherViewer", "be nice"),
        e(300, "ModOne", "ban", "spammer42", "came back to spam"),
        e(60, "ModTwo", "slowoff", "", ""),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_read_like_sentences() {
        let ev = serde_json::json!({
            "action": "timeout", "moderator_user_login": "mod", "moderator_user_name": "Mod",
            "timeout": {"user_login": "someone", "reason": "spam", "expires_at": "2026-11-17T20:09:59Z"}
        });
        let t0 = chrono::DateTime::parse_from_rfc3339("2026-11-17T20:00:00Z")
            .unwrap()
            .timestamp();
        let e = from_moderate(&ev, t0).unwrap();
        assert_eq!(e.describe(), "Mod timed out someone for 10m: spam");
        let del = serde_json::json!({
            "action": "delete", "moderator_user_name": "Mod",
            "delete": {"user_login": "someone", "message_id": "1", "message_body": "bad words"}
        });
        assert_eq!(
            from_moderate(&del, t0).unwrap().describe(),
            "Mod deleted a message of someone: bad words"
        );
        let slow = serde_json::json!({"action": "slow", "moderator_user_name": "Mod", "slow": {"wait_time_seconds": 30}});
        assert_eq!(
            from_moderate(&slow, t0).unwrap().describe(),
            "Mod turned on slow mode: 30s"
        );
        let terms = serde_json::json!({
            "action": "add_blocked_term", "moderator_user_name": "Mod",
            "automod_terms": {"action": "add", "list": "blocked", "terms": ["bad", "worse"], "from_automod": false}
        });
        assert_eq!(
            from_moderate(&terms, t0).unwrap().describe(),
            "Mod blocked a term: bad, worse"
        );
        let shared = serde_json::json!({"action": "shared_chat_ban", "moderator_user_name": "Mod", "shared_chat_ban": {"user_login": "x", "reason": ""}});
        assert_eq!(
            from_moderate(&shared, t0).unwrap().describe(),
            "Mod banned x (shared chat)"
        );
    }

    #[test]
    fn saved_lines_read_back() {
        let a = Entry {
            time: 1,
            moderator: "m".into(),
            action: "ban".into(),
            target: "t".into(),
            detail: "d".into(),
        };
        let text = format!("{}\nnot json\n", serde_json::to_string(&a).unwrap());
        assert_eq!(parse_lines(&text), [a]);
        let mut all = vec![Entry::default(); MAX_KEPT + 3];
        trim(&mut all);
        assert_eq!(all.len(), MAX_KEPT);
    }
}
