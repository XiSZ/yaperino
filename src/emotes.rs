//! Emotes and badges for the chat tabs, following chatterino7 (SevenTV/chatterino7):
//!
//! - Twitch emotes come with each message (the `emotes` tag).
//! - Third-party emotes are found by word, in chatterino7's order
//!   (src/messages/MessageBuilder.cpp): the channel's FFZ, BTTV and 7TV emotes,
//!   then the global FFZ, BTTV and 7TV emotes.
//! - URLs follow SeventvAPI.cpp / SeventvEmotes.cpp, BttvEmotes.cpp and FfzEmotes.cpp.
//! - Twitch badges come from Helix and need the signed-in account.

use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

pub const SEVENTV_GLOBAL: &str = "https://7tv.io/v3/emote-sets/global";
pub const SEVENTV_USER: &str = "https://7tv.io/v3/users/twitch/";
pub const BTTV_GLOBAL: &str = "https://api.betterttv.net/3/cached/emotes/global";
pub const BTTV_USER: &str = "https://api.betterttv.net/3/cached/users/twitch/";
pub const BTTV_CDN: &str = "https://cdn.betterttv.net/emote/";
pub const FFZ_GLOBAL: &str = "https://api.frankerfacez.com/v1/set/global";
pub const FFZ_ROOM: &str = "https://api.frankerfacez.com/v1/room/id/";
pub const TWITCH_EMOTE_CDN: &str = "https://static-cdn.jtvnw.net/emoticons/v2/";
pub const HELIX_BADGES: &str = "https://api.twitch.tv/helix/chat/badges";
/// Emotes are drawn at most this tall (Chatterino's 1x emote size is 28 px).
pub const EMOTE_HEIGHT: f32 = 28.0;
pub const BADGE_SIZE: f32 = 18.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    Twitch,
    Ffz,
    Bttv,
    SevenTv,
    Kick,
    YouTube,
}

impl Provider {
    pub fn label(self) -> &'static str {
        match self {
            Provider::Twitch => "Twitch",
            Provider::Ffz => "FFZ",
            Provider::Bttv => "BTTV",
            Provider::SevenTv => "7TV",
            Provider::Kick => "Kick",
            Provider::YouTube => "YouTube",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Emote {
    pub name: String,
    pub url: String,
    pub provider: Provider,
}

impl Emote {
    /// The id inside the emote's picture address (`.../emote/<id>/...`, `.../emoticons/v2/<id>/...`,
    /// `.../emotes/<id>/...`).
    pub fn id(&self) -> Option<&str> {
        for marker in ["/emote/", "/emoticons/v2/", "/emotes/"] {
            if let Some(i) = self.url.find(marker) {
                let rest = &self.url[i + marker.len()..];
                let id = rest.split(['/', '?']).next().unwrap_or("");
                if !id.is_empty() {
                    return Some(id);
                }
            }
        }
        None
    }

    /// The emote's own page, as Chatterino's "Open emote link": 7TV, BetterTTV and
    /// FrankerFaceZ pages; Twitch and Kick emotes have no page, so their large picture.
    pub fn page(&self) -> String {
        match (self.provider, self.id()) {
            (Provider::SevenTv, Some(id)) => format!("https://7tv.app/emotes/{id}"),
            (Provider::Bttv, Some(id)) => format!("https://betterttv.com/emotes/{id}"),
            (Provider::Ffz, Some(id)) => format!("https://www.frankerfacez.com/emoticon/{id}"),
            _ => self.large_url(),
        }
    }

    /// The largest picture (Twitch 3.0, 7TV 4x, BTTV 3x, FFZ 4).
    pub fn large_url(&self) -> String {
        let u = &self.url;
        match self.provider {
            Provider::Twitch => u.replace("/1.0", "/3.0"),
            Provider::SevenTv => u.replace("/1x.", "/4x."),
            Provider::Bttv => u.replace("/1x", "/3x"),
            Provider::Ffz => {
                if u.ends_with("/1") {
                    format!("{}4", &u[..u.len() - 1])
                } else {
                    u.clone()
                }
            }
            _ => u.clone(),
        }
    }
}

pub type EmoteMap = HashMap<String, Emote>;

/// One provider's emotes for a scope (global or one channel).
#[derive(Default, Clone)]
pub struct Set {
    pub ffz: EmoteMap,
    pub bttv: EmoteMap,
    pub seventv: EmoteMap,
}

impl Set {
    pub fn len(&self) -> usize {
        self.ffz.len() + self.bttv.len() + self.seventv.len()
    }
    /// chatterino7 order inside a scope: FFZ, BTTV, 7TV (skipping switched-off providers).
    fn get(&self, word: &str, off: [bool; 3]) -> Option<&Emote> {
        (!off[0])
            .then(|| self.ffz.get(word))
            .flatten()
            .or_else(|| (!off[1]).then(|| self.bttv.get(word)).flatten())
            .or_else(|| (!off[2]).then(|| self.seventv.get(word)).flatten())
    }
}

#[derive(Default)]
pub struct Store {
    pub global: Set,
    /// By channel login.
    pub channels: HashMap<String, Set>,
    /// "set/version" -> (image url, title).
    pub global_badges: HashMap<String, (String, String)>,
    pub channel_badges: HashMap<String, HashMap<String, (String, String)>>,
    /// Providers switched off in Settings > Emotes: FFZ, BTTV, 7TV.
    pub off: [bool; 3],
    /// Twitch emotes the signed-in account can use (global, subscriptions, follower...).
    pub user_twitch: Vec<(String, Emote)>,
    /// Kick emote sets per tab: (set name, emotes).
    pub kick_sets: HashMap<String, Vec<(String, Vec<Emote>)>>,
}

impl Store {
    /// Channel emotes first, then global ones.
    pub fn find(&self, channel: &str, word: &str) -> Option<&Emote> {
        self.channels
            .get(channel)
            .and_then(|s| s.get(word, self.off))
            .or_else(|| self.global.get(word, self.off))
    }

    pub fn badge(&self, channel: &str, set: &str, version: &str) -> Option<&(String, String)> {
        let key = format!("{set}/{version}");
        self.channel_badges
            .get(channel)
            .and_then(|m| m.get(&key))
            .or_else(|| self.global_badges.get(&key))
    }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .http_status_as_error(false)
        .user_agent("Yaperino/0.3")
        .build()
        .into()
}

fn get_json(url: &str) -> Option<Value> {
    let mut r = agent().get(url).call().ok()?;
    if r.status() != 200 {
        return None;
    }
    r.body_mut()
        .with_config()
        .limit(32 * 1024 * 1024)
        .read_json()
        .ok()
}

// ------------------------------------------------------------------ 7TV

/// SeventvEmotes.cpp: "https:" + host.url + "/" + the first WEBP file's name (1x).
fn seventv_emotes(list: &[Value]) -> EmoteMap {
    let mut out = EmoteMap::new();
    for active in list {
        let Some(name) = active.get("name").and_then(Value::as_str) else {
            continue;
        };
        let Some(host) = active.get("data").and_then(|d| d.get("host")) else {
            continue;
        };
        let base = host.get("url").and_then(Value::as_str).unwrap_or("");
        let file = host
            .get("files")
            .and_then(Value::as_array)
            .and_then(|files| {
                files
                    .iter()
                    .find(|f| f.get("format").and_then(Value::as_str) == Some("WEBP"))
                    .and_then(|f| f.get("name").and_then(Value::as_str))
            });
        if base.is_empty() {
            continue;
        }
        let url = format!("https:{base}/{}", file.unwrap_or("1x.webp"));
        out.insert(
            name.to_string(),
            Emote {
                name: name.to_string(),
                url,
                provider: Provider::SevenTv,
            },
        );
    }
    out
}

fn seventv_global() -> EmoteMap {
    get_json(SEVENTV_GLOBAL)
        .and_then(|v| {
            v.get("emotes")
                .and_then(Value::as_array)
                .map(|a| seventv_emotes(a))
        })
        .unwrap_or_default()
}

fn seventv_channel(room_id: &str) -> EmoteMap {
    get_json(&format!("{SEVENTV_USER}{room_id}"))
        .and_then(|v| {
            v.get("emote_set")
                .and_then(|s| s.get("emotes"))
                .and_then(Value::as_array)
                .map(|a| seventv_emotes(a))
        })
        .unwrap_or_default()
}

// ------------------------------------------------------------------ BTTV

fn bttv_emotes(list: &[Value], out: &mut EmoteMap) {
    for e in list {
        let (Some(id), Some(code)) = (
            e.get("id").and_then(Value::as_str),
            e.get("code").and_then(Value::as_str),
        ) else {
            continue;
        };
        out.insert(
            code.to_string(),
            Emote {
                name: code.to_string(),
                url: format!("{BTTV_CDN}{id}/1x.webp"),
                provider: Provider::Bttv,
            },
        );
    }
}

fn bttv_global() -> EmoteMap {
    let mut out = EmoteMap::new();
    if let Some(Value::Array(a)) = get_json(BTTV_GLOBAL) {
        bttv_emotes(&a, &mut out);
    }
    out
}

fn bttv_channel(room_id: &str) -> EmoteMap {
    let mut out = EmoteMap::new();
    if let Some(v) = get_json(&format!("{BTTV_USER}{room_id}")) {
        for key in ["channelEmotes", "sharedEmotes"] {
            if let Some(a) = v.get(key).and_then(Value::as_array) {
                bttv_emotes(a, &mut out);
            }
        }
    }
    out
}

// ------------------------------------------------------------------ FFZ

fn ffz_url(urls: &Value) -> Option<String> {
    let u = urls.get("1").and_then(Value::as_str)?;
    Some(if let Some(rest) = u.strip_prefix("//") {
        format!("https://{rest}")
    } else {
        u.to_string()
    })
}

/// FfzEmotes.cpp: every set's emoticons, preferring the animated image.
fn ffz_sets(v: &Value, only: Option<&[String]>) -> EmoteMap {
    let mut out = EmoteMap::new();
    let Some(sets) = v.get("sets").and_then(Value::as_object) else {
        return out;
    };
    for (id, set) in sets {
        if only.is_some_and(|o| !o.contains(id)) {
            continue;
        }
        for e in set
            .get("emoticons")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(name) = e.get("name").and_then(Value::as_str) else {
                continue;
            };
            let urls = e
                .get("animated")
                .filter(|a| a.is_object())
                .or_else(|| e.get("urls"));
            let Some(url) = urls.and_then(ffz_url) else {
                continue;
            };
            out.insert(
                name.to_string(),
                Emote {
                    name: name.to_string(),
                    url,
                    provider: Provider::Ffz,
                },
            );
        }
    }
    out
}

fn ffz_global() -> EmoteMap {
    get_json(FFZ_GLOBAL)
        .map(|v| {
            let defaults: Vec<String> = v
                .get("default_sets")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .map(|x| x.to_string().trim_matches('"').to_string())
                        .collect()
                })
                .unwrap_or_default();
            ffz_sets(&v, Some(&defaults))
        })
        .unwrap_or_default()
}

fn ffz_channel(room_id: &str) -> EmoteMap {
    get_json(&format!("{FFZ_ROOM}{room_id}"))
        .map(|v| ffz_sets(&v, None))
        .unwrap_or_default()
}

// ------------------------------------------------------------------ loading

pub fn load_global() -> Set {
    let ffz = std::thread::spawn(ffz_global);
    let bttv = std::thread::spawn(bttv_global);
    let seventv = seventv_global();
    Set {
        ffz: ffz.join().unwrap_or_default(),
        bttv: bttv.join().unwrap_or_default(),
        seventv,
    }
}

pub fn load_channel(room_id: &str) -> Set {
    let (a, b) = (room_id.to_string(), room_id.to_string());
    let ffz = std::thread::spawn(move || ffz_channel(&a));
    let bttv = std::thread::spawn(move || bttv_channel(&b));
    let seventv = seventv_channel(room_id);
    Set {
        ffz: ffz.join().unwrap_or_default(),
        bttv: bttv.join().unwrap_or_default(),
        seventv,
    }
}

/// Twitch chat badges from Helix (needs a token). `room_id` None = global badges.
pub fn load_badges(
    client_id: &str,
    token: &str,
    room_id: Option<&str>,
) -> HashMap<String, (String, String)> {
    let url = match room_id {
        Some(id) => format!("{HELIX_BADGES}?broadcaster_id={id}"),
        None => format!("{HELIX_BADGES}/global"),
    };
    let mut out = HashMap::new();
    let resp = agent()
        .get(&url)
        .header("Authorization", &format!("Bearer {token}"))
        .header("Client-Id", client_id)
        .call();
    let Ok(mut r) = resp else { return out };
    if r.status() != 200 {
        return out;
    }
    let Ok(v) = r.body_mut().read_json::<Value>() else {
        return out;
    };
    for set in v
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let set_id = set.get("set_id").and_then(Value::as_str).unwrap_or("");
        for ver in set
            .get("versions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let id = ver.get("id").and_then(Value::as_str).unwrap_or("");
            let img = ver
                .get("image_url_1x")
                .and_then(Value::as_str)
                .unwrap_or("");
            let title = ver.get("title").and_then(Value::as_str).unwrap_or(set_id);
            if !img.is_empty() {
                out.insert(
                    format!("{set_id}/{id}"),
                    (img.to_string(), title.to_string()),
                );
            }
        }
    }
    out
}

/// Twitch emotes the account can use (Helix chat/emotes/user, scope user:read:emotes),
/// grouped by type (globals, subscriptions, follower, bits...).
pub fn load_user_emotes(client_id: &str, token: &str, user_id: &str) -> Vec<(String, Emote)> {
    let mut out = Vec::new();
    let mut cursor = String::new();
    for _ in 0..30 {
        let mut url = format!("https://api.twitch.tv/helix/chat/emotes/user?user_id={user_id}");
        if !cursor.is_empty() {
            url.push_str(&format!("&after={cursor}"));
        }
        let Ok(mut r) = agent()
            .get(&url)
            .header("Authorization", &format!("Bearer {token}"))
            .header("Client-Id", client_id)
            .call()
        else {
            break;
        };
        if r.status() != 200 {
            break;
        }
        let Ok(v) = r
            .body_mut()
            .with_config()
            .limit(32 * 1024 * 1024)
            .read_json::<Value>()
        else {
            break;
        };
        for e in v
            .get("data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let (Some(id), Some(name)) = (
                e.get("id").and_then(Value::as_str),
                e.get("name").and_then(Value::as_str),
            ) else {
                continue;
            };
            let kind = match e.get("emote_type").and_then(Value::as_str).unwrap_or("") {
                "globals" | "" => "Twitch global".to_string(),
                "subscriptions" => "Twitch subscriptions".to_string(),
                "follower" => "Twitch follower".to_string(),
                "bitstier" => "Twitch bits".to_string(),
                other => format!("Twitch {other}"),
            };
            out.push((
                kind,
                Emote {
                    name: name.to_string(),
                    url: twitch_emote_url(id),
                    provider: Provider::Twitch,
                },
            ));
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
    out
}

/// Kick emote sets for a channel (https://kick.com/emotes/{slug}, as Moltorino's
/// privateEmotesInChannel): the channel's own set, then Global and Emojis.
pub fn load_kick_emotes(slug: &str) -> Vec<(String, Vec<Emote>)> {
    let Some(Ok((200, _, body))) = crate::platforms::curl_full(
        "GET",
        &format!("https://kick.com/emotes/{}", slug.to_lowercase()),
        &[],
        None,
        false,
    ) else {
        return Vec::new();
    };
    let Ok(Value::Array(sets)) = serde_json::from_str::<Value>(&body) else {
        return Vec::new();
    };
    sets.iter()
        .map(|s| {
            let label = s
                .get("slug")
                .or_else(|| s.get("name"))
                .and_then(Value::as_str)
                .map(|n| {
                    if s.get("slug").is_some() {
                        format!("Kick {n}")
                    } else {
                        format!("Kick {n}")
                    }
                })
                .unwrap_or_else(|| "Kick".into());
            let emotes = s
                .get("emotes")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|e| {
                    let id = e.get("id")?.as_u64()?;
                    let name = e.get("name")?.as_str()?.to_string();
                    Some(Emote {
                        url: format!("{}{id}/fullsize", crate::platforms::KICK_EMOTE),
                        name,
                        provider: Provider::Kick,
                    })
                })
                .collect();
            (label, emotes)
        })
        .collect()
}

pub fn twitch_emote_url(id: &str) -> String {
    format!("{TWITCH_EMOTE_CDN}{id}/default/dark/1.0")
}

// ------------------------------------------------------------------ messages

/// A piece of a chat message: text or an emote.
#[derive(Clone, Debug)]
pub enum Seg {
    Text(String),
    Emote(Emote),
}

/// Twitch's `emotes` tag: "25:0-4,12-16/1902:6-10" -> (start, end inclusive, id),
/// positions in characters.
pub fn parse_emote_tag(tag: &str) -> Vec<(usize, usize, String)> {
    let mut out = Vec::new();
    for part in tag.split('/').filter(|p| !p.is_empty()) {
        let Some((id, ranges)) = part.split_once(':') else {
            continue;
        };
        for r in ranges.split(',') {
            if let Some((a, b)) = r.split_once('-') {
                if let (Ok(a), Ok(b)) = (a.parse::<usize>(), b.parse::<usize>()) {
                    out.push((a, b, id.to_string()));
                }
            }
        }
    }
    out.sort_by_key(|x| x.0);
    out
}

/// Splits a message into text and emotes: Twitch emotes by position, then
/// third-party emotes by whole word.
pub fn segments(
    text: &str,
    twitch: &[(usize, usize, String)],
    store: &Store,
    channel: &str,
) -> Vec<Seg> {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<Seg> = Vec::new();
    let push_text = |out: &mut Vec<Seg>, s: &str| {
        // third-party emotes by word, keeping the spaces
        let mut buf = String::new();
        for (i, word) in s.split(' ').enumerate() {
            if i > 0 {
                buf.push(' ');
            }
            match store.find(channel, word) {
                Some(e) if !word.is_empty() => {
                    if !buf.is_empty() {
                        push_seg_text(out, std::mem::take(&mut buf));
                    }
                    out.push(Seg::Emote(e.clone()));
                }
                _ => buf.push_str(word),
            }
        }
        if !buf.is_empty() {
            push_seg_text(out, buf);
        }
    };
    let mut pos = 0usize;
    for (a, b, id) in twitch {
        if *a < pos || *b >= chars.len() || a > b {
            continue;
        }
        let before: String = chars[pos..*a].iter().collect();
        push_text(&mut out, &before);
        let name: String = chars[*a..=*b].iter().collect();
        out.push(Seg::Emote(Emote {
            name,
            url: twitch_emote_url(id),
            provider: Provider::Twitch,
        }));
        pos = b + 1;
    }
    let rest: String = chars[pos.min(chars.len())..].iter().collect();
    push_text(&mut out, &rest);
    out
}

/// Twitch emotes in your own message, found by whole word in the emotes your account
/// can use. Twitch doesn't send your own messages back with emote positions, so
/// without this they'd show as plain words (Chatterino finds them the same way).
pub fn own_twitch_positions(text: &str, usable: &[(String, Emote)]) -> Vec<(usize, usize, String)> {
    let mut out = Vec::new();
    let mut start = 0usize; // in characters, as Twitch counts them
    for word in text.split(' ') {
        let len = word.chars().count();
        if len > 0 {
            if let Some(id) = usable
                .iter()
                .find(|(_, e)| e.name == word)
                .and_then(|(_, e)| e.id())
            {
                out.push((start, start + len - 1, id.to_string()));
            }
        }
        start += len + 1;
    }
    out
}

/// Kick only shows an emote when the message carries it as `[emote:<id>:<name>]` (what
/// kick.com's own emote picker sends); a typed emote name stays text. Turns the channel's
/// Kick emote names into that form before sending.
pub fn kick_emote_codes(text: &str, sets: Option<&Vec<(String, Vec<Emote>)>>) -> String {
    let Some(sets) = sets else {
        return text.to_string();
    };
    text.split(' ')
        .map(|w| {
            sets.iter()
                .flat_map(|(_, list)| list.iter())
                .find(|e| !w.is_empty() && e.name == w)
                .and_then(|e| e.id().map(|id| format!("[emote:{id}:{}]", e.name)))
                .unwrap_or_else(|| w.to_string())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn push_seg_text(out: &mut Vec<Seg>, s: String) {
    if let Some(Seg::Text(t)) = out.last_mut() {
        t.push_str(&s);
    } else {
        out.push(Seg::Text(s));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_message_twitch_emotes_by_word() {
        let usable = vec![(
            "global".to_string(),
            Emote {
                name: "Kappa".into(),
                url: twitch_emote_url("25"),
                provider: Provider::Twitch,
            },
        )];
        // "héllo" has a non-ASCII letter: positions count characters, not bytes
        let pos = own_twitch_positions("héllo Kappa  Kappa x", &usable);
        assert_eq!(
            pos,
            vec![(6, 10, "25".to_string()), (13, 17, "25".to_string())]
        );
        let store = Store::default();
        let segs = segments("héllo Kappa  Kappa x", &pos, &store, "c");
        assert_eq!(
            segs.iter()
                .filter(|s| matches!(s, Seg::Emote(e) if e.name == "Kappa"))
                .count(),
            2
        );
    }

    #[test]
    fn kick_emote_names_become_codes() {
        let sets = vec![(
            "channel".to_string(),
            vec![Emote {
                name: "KEKW".into(),
                url: format!("{}123/fullsize", crate::platforms::KICK_EMOTE),
                provider: Provider::Kick,
            }],
        )];
        assert_eq!(
            kick_emote_codes("lol KEKW  kekw", Some(&sets)),
            "lol [emote:123:KEKW]  kekw"
        );
        assert_eq!(kick_emote_codes("KEKW", None), "KEKW");
    }

    #[test]
    fn twitch_positions_and_words() {
        let mut store = Store::default();
        store.global.bttv.insert(
            "catJAM".into(),
            Emote {
                name: "catJAM".into(),
                url: "u".into(),
                provider: Provider::Bttv,
            },
        );
        let tags = parse_emote_tag("25:0-4");
        let segs = segments("Kappa hello catJAM bye", &tags, &store, "x");
        let kinds: Vec<String> = segs
            .iter()
            .map(|s| match s {
                Seg::Text(t) => format!("T[{t}]"),
                Seg::Emote(e) => format!("E[{}]", e.name),
            })
            .collect();
        assert_eq!(
            kinds,
            vec!["E[Kappa]", "T[ hello ]", "E[catJAM]", "T[ bye]"]
        );
    }

    #[test]
    fn channel_emote_wins() {
        let mut store = Store::default();
        let e = |p| Emote {
            name: "x".into(),
            url: "u".into(),
            provider: p,
        };
        store.global.ffz.insert("Pog".into(), e(Provider::Ffz));
        store
            .channels
            .entry("chan".into())
            .or_default()
            .seventv
            .insert("Pog".into(), e(Provider::SevenTv));
        assert_eq!(
            store.find("chan", "Pog").unwrap().provider,
            Provider::SevenTv
        );
        assert_eq!(store.find("other", "Pog").unwrap().provider, Provider::Ffz);
    }

    /// Network: `cargo test -- --ignored loads_global_emotes --nocapture`
    #[test]
    #[ignore]
    fn loads_global_emotes() {
        let g = load_global();
        println!(
            "global: ffz {} bttv {} 7tv {}",
            g.ffz.len(),
            g.bttv.len(),
            g.seventv.len()
        );
        let mut names: Vec<&String> = g
            .seventv
            .keys()
            .chain(g.bttv.keys())
            .chain(g.ffz.keys())
            .collect();
        names.sort();
        println!("names: {:?}", names);
        if let Some(e) = g.seventv.values().next() {
            println!("7tv sample url: {}", e.url);
        }
        assert!(g.len() > 0);
    }

    /// Network: channel emotes and badges for a real channel (a large channel, id 71092938),
    /// badges with the Moltorino account. `cargo test -- --ignored channel_emotes_and_badges --nocapture`
    #[test]
    #[ignore]
    fn channel_emotes_and_badges() {
        let c = load_channel("71092938");
        println!(
            "channel: ffz {} bttv {} 7tv {}",
            c.ffz.len(),
            c.bttv.len(),
            c.seventv.len()
        );
        if let Some(acc) = crate::account::test_account() {
            let g = load_badges(&acc.client_id, &acc.token, None);
            println!("global badges: {}", g.len());
            assert!(g.contains_key("moderator/1"));
        }
    }

    /// Network: `cargo test -- --ignored kick_emote_sets --nocapture`
    #[test]
    #[ignore]
    fn kick_emote_sets() {
        let sets = load_kick_emotes("xqc");
        for (name, list) in &sets {
            println!("{name}: {}", list.len());
        }
        assert!(sets.iter().any(|(_, l)| !l.is_empty()));
    }
}
