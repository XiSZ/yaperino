//! Built-in sample streams for screenshot mode (no network, made-up channels).
//! Viewer counts and uptimes mirror the spread seen in the person's picture.

use crate::chat::ChatLine;
use crate::model::{now_unix, Stream};
use crate::platforms::Platform;

pub fn streams() -> Vec<Stream> {
    let now = now_unix();
    let rows: [(&str, u64, i64, &str, &str); 18] = [
        (
            "Northwind",
            4991,
            270,
            "RANKED GRIND UNTIL DIAMOND | !socials",
            "Tactical Shooter",
        ),
        (
            "PixelMaple",
            1839,
            258,
            "Cozy morning stream, come say hi !schedule",
            "Just Chatting",
        ),
        (
            "Quillfox",
            1755,
            40,
            "DAY 15 of the 100 missions challenge !challenges",
            "Just Chatting",
        ),
        (
            "Bramblecat",
            1467,
            222,
            "♡ happy Sunday ♡ | !discord !wishlist !socials",
            "Horror Adventure",
        ),
        (
            "Tidewalker",
            947,
            180,
            "trying to stay calm in competitive !new",
            "Counter Play",
        ),
        (
            "Lumenberry",
            838,
            96,
            "NEW sprites night! 3x new emotes today",
            "Battle Royale",
        ),
        (
            "Marigold",
            805,
            270,
            "[SUBATHON DAY 12] leveling a new hunter from scratch",
            "Fantasy MMO",
        ),
        (
            "Orbitjay",
            779,
            144,
            "My own game | creator code in chat | !socials",
            "Survival Shooter",
        ),
        (
            "Emberlyn",
            717,
            2652,
            "✨ 24/7 hangout ✨ more of me !socials !tip",
            "Just Chatting",
        ),
        (
            "Velvetoak",
            614,
            372,
            "MY FIRST ADVENTURE GAME ⚔ WE FINISH TODAY!! (Day 7)",
            "Classic Adventure",
        ),
        (
            "Snowpetal",
            558,
            222,
            "ret pala leveling into hunter !journey",
            "Fantasy MMO",
        ),
        (
            "Fernlight",
            470,
            27,
            "Will the stick hit again?? | @fernlight for socials",
            "Open World RPG",
        ),
        (
            "Rainhollow",
            324,
            180,
            "Finishing the diving game first playthrough???",
            "Ocean Sim",
        ),
        (
            "Cobaltwren",
            272,
            2340,
            "Hotel stream: 5 YEARS LIVE 24/7! !record !links",
            "Just Chatting",
        ),
        (
            "Skylark",
            193,
            9,
            "✳ Gaming and yapping for your entertainment! ✳ | !merch",
            "Just Chatting",
        ),
        (
            "Cinderbear",
            173,
            2364,
            "#1 cozy community, !games !seasonal",
            "Just Chatting",
        ),
        (
            "Hazelnova",
            151,
            75,
            "late night puzzles and lofi",
            "Puzzle",
        ),
        (
            "Driftmoss",
            98,
            33,
            "learning to speedrun, be nice",
            "Platformer",
        ),
    ];
    rows.iter()
        .enumerate()
        .map(|(i, (name, viewers, minutes, title, game))| Stream {
            login: name.to_lowercase(),
            display_name: name.to_string(),
            title: title.to_string(),
            game: game.to_string(),
            viewers: *viewers,
            started: now - minutes * 60 - 30,
            status_changed: now - minutes * 60 - 30,
            list_index: i,
            platform: crate::platforms::Platform::Twitch,
            group: name.to_lowercase(),
        })
        .collect()
}

pub fn chat_lines() -> Vec<ChatLine> {
    // emote positions like Twitch's `emotes` tag; "Kappa" is Twitch emote 25
    let m = |t: &str, n: &str, c: [u8; 3], x: &str, emotes: &str| ChatLine::Msg {
        time: t.into(),
        name: n.into(),
        color: Some(c),
        text: x.into(),
        action: false,
        own: false,
        emotes: crate::emotes::parse_emote_tag(emotes),
        badges: Vec::new(),
        segs: Vec::new(),
        platform: crate::platforms::Platform::Twitch,
        msg_id: String::new(),
        user_id: String::new(),
        reply: None,
    };
    vec![
        ChatLine::System("Sample chat (screenshot mode)".into()),
        m(
            "18:02",
            "viewer_one",
            [255, 99, 71],
            "hello chat Kappa",
            "25:11-15",
        ),
        m(
            "18:02",
            "AnotherViewer",
            [30, 144, 255],
            "good stream today EZ Clap",
            "",
        ),
        m(
            "18:03",
            "third_person",
            [50, 205, 50],
            "lets go! catJAM",
            "",
        ),
        other(
            Platform::Kick,
            "18:03",
            "kick_viewer",
            [83, 252, 24],
            "hello from Kick",
        ),
        m(
            "18:03",
            "raid_fan",
            [0, 206, 209],
            "come over to #pixelmaple after this",
            "",
        ),
        other(
            Platform::YouTube,
            "18:04",
            "YouTube Viewer",
            [255, 80, 80],
            "watching on YouTube",
        ),
    ]
}

fn other(platform: Platform, t: &str, n: &str, c: [u8; 3], x: &str) -> ChatLine {
    ChatLine::Msg {
        time: t.into(),
        name: n.into(),
        color: Some(c),
        text: x.into(),
        action: false,
        own: false,
        emotes: Vec::new(),
        badges: Vec::new(),
        segs: Vec::new(),
        platform,
        msg_id: String::new(),
        user_id: String::new(),
        reply: None,
    }
}

/// Test-only sample streams on Kick and YouTube (for the platform sections).
pub fn other_platform_streams() -> Vec<Stream> {
    let now = now_unix();
    let rows: [(Platform, &str, u64, i64, &str, &str); 3] = [
        (
            Platform::Kick,
            "Quillfox",
            751,
            228,
            "DAY 15 of the 100 missions challenge",
            "IRL",
        ),
        (
            Platform::Kick,
            "Driftmoss",
            120,
            40,
            "speedrun practice on Kick",
            "Platformer",
        ),
        (
            Platform::YouTube,
            "Northwind",
            2210,
            270,
            "RANKED GRIND simulcast",
            "Gaming",
        ),
    ];
    rows.iter()
        .enumerate()
        .map(|(i, (p, name, viewers, minutes, title, game))| Stream {
            login: format!("{}:{}", p.label().to_lowercase(), name.to_lowercase()),
            display_name: name.to_string(),
            title: title.to_string(),
            game: game.to_string(),
            viewers: *viewers,
            started: now - minutes * 60,
            status_changed: now - minutes * 60,
            list_index: 100 + i,
            platform: *p,
            group: name.to_lowercase(),
        })
        .collect()
}
