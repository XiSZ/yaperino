//! One live stream and the rules for showing and ordering the list.
//! The rules follow Chatty's Live Streams window (LiveStreamsDialog.java and
//! LiveStreamsList.java); MECHANICS.md lists them with their sources.

use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub struct Stream {
    pub login: String,
    pub display_name: String,
    pub title: String,
    pub game: String,
    pub viewers: u64,
    /// Unix seconds the stream started.
    pub started: i64,
    /// Unix seconds the stream went live or last changed its title/game
    /// (Chatty's "status change time"). Filled in by the window.
    pub status_changed: i64,
    /// Order the channel had in the saved list (lower = added more recently).
    #[allow(dead_code)]
    pub list_index: usize,
    /// Twitch, Kick, YouTube or TikTok.
    pub platform: crate::platforms::Platform,
    /// The Twitch channel this stream belongs to (its chat tab); for Twitch the login itself.
    pub group: String,
}

/// A stream changed (went live, new title or game) less than this long ago is
/// marked as new: Chatty draws its rule differently (STREAMINFO_NEW_TIME).
pub const NEW_SECONDS: i64 = 180;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SortMode {
    Recent,
    Uptime,
    Name,
    Game,
    Viewers,
}

impl SortMode {
    /// In Chatty's menu order.
    pub const ALL: [SortMode; 5] = [
        SortMode::Recent,
        SortMode::Uptime,
        SortMode::Name,
        SortMode::Game,
        SortMode::Viewers,
    ];

    /// Chatty's labels (Strings.properties streams.sorting.*); the window title uses them too.
    pub fn label(self) -> &'static str {
        match self {
            SortMode::Recent => "Changed (Most Recent)",
            SortMode::Uptime => "Uptime (Newest)",
            SortMode::Name => "Name",
            SortMode::Game => "Game",
            SortMode::Viewers => "Viewers",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            SortMode::Recent => "Stream most recently started or title/game changed",
            SortMode::Uptime => "Stream most recently started",
            SortMode::Name => "Channel name (alphabetically)",
            SortMode::Game => "Game name (alphabetically)",
            SortMode::Viewers => "Highest amount of viewers",
        }
    }
}

/// Favourite channels and games, and whether they go to the top.
#[derive(Default)]
pub struct Favs<'a> {
    pub channels: Option<&'a HashSet<String>>,
    pub games: Option<&'a HashSet<String>>,
    pub first: bool,
}

impl Favs<'_> {
    pub fn is_fav(&self, s: &Stream) -> bool {
        self.channels.is_some_and(|c| c.contains(&s.login))
            || self
                .games
                .is_some_and(|g| !s.game.is_empty() && g.contains(&s.game))
    }
}

/// Chatty's comparators: ties are broken by channel name; with "Favorites
/// first", favourite channels and favourite games come before the rest.
pub fn sort_streams(streams: &mut [Stream], mode: SortMode, favs: &Favs<'_>) {
    use std::cmp::Ordering;
    let by_mode = |a: &Stream, b: &Stream| -> Ordering {
        let name = a.login.cmp(&b.login);
        match mode {
            SortMode::Name => name,
            // smaller "time since" first = most recent first
            SortMode::Recent => b.status_changed.cmp(&a.status_changed).then(name),
            SortMode::Uptime => b.started.cmp(&a.started).then(name),
            // no game sorts last
            SortMode::Game => match (a.game.is_empty(), b.game.is_empty()) {
                (true, true) => name,
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                _ => a.game.cmp(&b.game).then(name),
            },
            SortMode::Viewers => b.viewers.cmp(&a.viewers).then(name),
        }
    };
    streams.sort_by(|a, b| {
        if favs.first {
            let (fa, fb) = (favs.is_fav(a), favs.is_fav(b));
            if fa != fb {
                return if fa {
                    Ordering::Less
                } else {
                    Ordering::Greater
                };
            }
        }
        by_mode(a, b)
    });
}
pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 4991 -> "4.991", 1234567 -> "1.234.567" (dot between thousands, as in the picture).
pub fn format_viewers(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    let len = digits.len();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            out.push('.');
        }
        out.push(ch);
    }
    out
}

/// Under an hour: whole minutes ("40m"). From an hour on: hours with one decimal ("4.5h", "44.2h").
pub fn format_uptime(seconds: i64) -> String {
    let seconds = seconds.max(0);
    if seconds < 3600 {
        format!("{}m", seconds / 60)
    } else {
        // round down to one decimal, so 4h59m shows 4.9h rather than 5.0h
        let tenths = seconds * 10 / 3600;
        format!("{}.{}h", tenths / 10, tenths % 10)
    }
}

/// "Kyedae (4.991 | 4.5h)"
pub fn header_text(s: &Stream, now: i64) -> String {
    format!(
        "{} ({} | {})",
        s.display_name,
        format_viewers(s.viewers),
        format_uptime(now - s.started)
    )
}

/// "2024-05-01T12:34:56Z" (fractions and +hh:mm allowed) -> Unix seconds.
pub fn parse_iso8601(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 19 {
        return None;
    }
    let num = |a: usize, z: usize| -> Option<i64> { s.get(a..z)?.parse::<i64>().ok() };
    let (y, mo, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (h, mi, sec) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    // days since 1970-01-01 (civil-from-days inverse)
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = if mo > 2 { mo - 3 } else { mo + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    let mut t = days * 86400 + h * 3600 + mi * 60 + sec;
    // optional offset after fractions
    let rest = s[19..].trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    if rest.len() >= 6 && (rest.starts_with('+') || rest.starts_with('-')) {
        let oh: i64 = rest[1..3].parse().unwrap_or(0);
        let om: i64 = rest[rest.len() - 2..].parse().unwrap_or(0);
        let off = oh * 3600 + om * 60;
        t -= if rest.starts_with('+') { off } else { -off };
    }
    Some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_like_the_picture() {
        assert_eq!(format_viewers(4991), "4.991");
        assert_eq!(format_viewers(947), "947");
        assert_eq!(format_viewers(1234567), "1.234.567");
        assert_eq!(format_uptime(40 * 60), "40m");
        assert_eq!(format_uptime(9 * 60 + 5), "9m");
        assert_eq!(format_uptime(4 * 3600 + 30 * 60), "4.5h");
        assert_eq!(format_uptime(44 * 3600 + 13 * 60), "44.2h");
    }

    #[test]
    fn parses_iso() {
        assert_eq!(parse_iso8601("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_iso8601("2024-05-01T12:34:56Z"), Some(1714566896));
        assert_eq!(parse_iso8601("2024-05-01T12:34:56.123Z"), Some(1714566896));
    }
}
