//! Twitch chat for all tabs over one shared connection.
//!
//! Signed out it joins anonymously (read only). Signed in it logs in with the
//! account (`PASS oauth:<token>`, `NICK <login>`) so messages can be sent.
//! Twitch does not echo your own messages back, so they are added locally with
//! the colour from USERSTATE / GLOBALUSERSTATE. After a reconnect every open
//! tab is joined again.

use std::collections::HashSet;
use std::net::TcpStream;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

pub const IRC_URL: &str = "wss://irc-ws.chat.twitch.tv:443";
/// Twitch's message length limit.
pub const MAX_MESSAGE: usize = 500;

#[derive(Clone, Debug)]
#[allow(dead_code)] // own marks this account's messages; kept for highlighting later
pub enum ChatLine {
    Msg {
        time: String,
        name: String,
        color: Option<[u8; 3]>,
        text: String,
        action: bool,
        own: bool,
        /// Twitch emote positions from the motes tag: (start, end, id), in characters.
        emotes: Vec<(usize, usize, String)>,
        /// Badges from the adges tag: (set, version).
        badges: Vec<(String, String)>,
        /// Text and emotes, filled in by the window when the line arrives.
        segs: Vec<crate::emotes::Seg>,
        /// Where the message came from (one tab can show several platforms).
        platform: crate::platforms::Platform,
        /// Twitch's message id and the sender's user id (tags id, user-id), for moderation.
        msg_id: String,
        user_id: String,
    },
    System(String),
}

/// What the chat thread reports back to the window.
#[derive(Debug)]
pub enum Event {
    /// A line for one channel (login, without '#').
    Line(String, ChatLine),
    /// A line for every open tab (connection state).
    All(ChatLine),
    /// The account's token was refused: the window should load it again.
    AuthFailed,
    /// Logged in as this name (None = anonymous).
    Connected(Option<String>),
    /// A channel's Twitch id (ROOMSTATE room-id), for its 7TV/BTTV/FFZ emotes and badges.
    Room(String, String),
}

pub enum Cmd {
    Join(String),
    Part(String),
    Send(String, String),
    /// (login, token) or None for anonymous.
    Auth(Option<(String, String)>),
}

pub struct Chat {
    tx: Sender<Cmd>,
    pub rx: Receiver<Event>,
}

impl Chat {
    pub fn start(ctx: egui::Context) -> Self {
        let (tx, cmd_rx) = std::sync::mpsc::channel();
        let (ev_tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || worker(cmd_rx, ev_tx, ctx));
        Self { tx, rx }
    }
    pub fn send(&self, cmd: Cmd) {
        let _ = self.tx.send(cmd);
    }
}

type Ws = WebSocket<MaybeTlsStream<TcpStream>>;

struct State {
    channels: HashSet<String>,
    auth: Option<(String, String)>,
    /// Our own name colour, display name and badges once Twitch tells us.
    color: Option<[u8; 3]>,
    badges: Vec<(String, String)>,
    display: Option<String>,
}

fn worker(cmds: Receiver<Cmd>, ev: Sender<Event>, ctx: egui::Context) {
    let mut st = State {
        channels: HashSet::new(),
        auth: None,
        color: None,
        badges: Vec::new(),
        display: None,
    };
    let emit = |e: Event| {
        let _ = ev.send(e);
        ctx.request_repaint();
    };
    let mut backoff = 0u64;
    loop {
        // wait until there is something to join
        while st.channels.is_empty() {
            match cmds.recv() {
                Ok(c) => apply_offline(&mut st, c, &emit),
                Err(_) => return,
            }
        }
        if backoff > 0 {
            emit(Event::All(ChatLine::System("Reconnecting...".into())));
            std::thread::sleep(Duration::from_secs(backoff));
        }
        match session(&mut st, &cmds, &emit) {
            Ok(Exit::Quit) => return,
            Ok(Exit::Reconnect) => backoff = 0,
            Err(e) => {
                emit(Event::All(ChatLine::System(format!("Disconnected: {e}"))));
                backoff = (backoff * 2).clamp(2, 30);
            }
        }
    }
}

fn apply_offline(st: &mut State, c: Cmd, emit: &impl Fn(Event)) {
    match c {
        Cmd::Join(ch) => {
            st.channels.insert(ch);
        }
        Cmd::Part(ch) => {
            st.channels.remove(&ch);
        }
        Cmd::Auth(a) => st.auth = a,
        Cmd::Send(ch, _) => emit(Event::Line(
            ch,
            ChatLine::System("Not connected yet; message not sent.".into()),
        )),
    }
}

enum Exit {
    Quit,
    Reconnect,
}

fn set_timeout(s: &MaybeTlsStream<TcpStream>) {
    let t = Some(Duration::from_millis(100));
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

fn text(ws: &mut Ws, line: String) -> Result<(), String> {
    ws.send(Message::Text(line.into()))
        .map_err(|e| e.to_string())
}

fn session(st: &mut State, cmds: &Receiver<Cmd>, emit: &impl Fn(Event)) -> Result<Exit, String> {
    let (mut ws, _) = tungstenite::connect(IRC_URL).map_err(|e| e.to_string())?;
    set_timeout(ws.get_ref());
    text(&mut ws, "CAP REQ :twitch.tv/tags twitch.tv/commands".into())?;
    let me = match &st.auth {
        Some((login, token)) => {
            text(&mut ws, format!("PASS oauth:{token}"))?;
            text(&mut ws, format!("NICK {login}"))?;
            Some(login.clone())
        }
        None => {
            text(&mut ws, "PASS SCHMOOPIIE".into())?;
            text(
                &mut ws,
                format!(
                    "NICK justinfan{}",
                    10000 + (crate::model::now_unix() % 80000)
                ),
            )?;
            None
        }
    };
    emit(Event::Connected(me.clone()));
    for ch in st.channels.clone() {
        text(&mut ws, format!("JOIN #{ch}"))?;
    }
    let mut last_ping = Instant::now();
    loop {
        // commands from the window
        loop {
            match cmds.try_recv() {
                Ok(Cmd::Join(ch)) => {
                    if st.channels.insert(ch.clone()) {
                        text(&mut ws, format!("JOIN #{ch}"))?;
                    }
                }
                Ok(Cmd::Part(ch)) => {
                    if st.channels.remove(&ch) {
                        text(&mut ws, format!("PART #{ch}"))?;
                    }
                }
                Ok(Cmd::Auth(a)) => {
                    if a != st.auth {
                        st.auth = a;
                        st.color = None;
                        st.display = None;
                        let _ = ws.close(None);
                        return Ok(Exit::Reconnect);
                    }
                }
                Ok(Cmd::Send(ch, msg)) => {
                    let msg: String = msg
                        .replace(['\r', '\n'], " ")
                        .chars()
                        .take(MAX_MESSAGE)
                        .collect();
                    if me.is_none() {
                        emit(Event::Line(
                            ch,
                            ChatLine::System(
                                "Sign in to send messages (right-click the live list > Account)."
                                    .into(),
                            ),
                        ));
                    } else if !msg.trim().is_empty() {
                        text(&mut ws, format!("PRIVMSG #{ch} :{msg}"))?;
                        let (body, action) = match msg.strip_prefix("/me ") {
                            Some(b) => (b.to_string(), true),
                            None => (msg.clone(), false),
                        };
                        let name = st
                            .display
                            .clone()
                            .or_else(|| me.clone())
                            .unwrap_or_default();
                        emit(Event::Line(
                            ch,
                            ChatLine::Msg {
                                time: clock(),
                                name,
                                color: st.color,
                                text: body,
                                action,
                                own: true,
                                emotes: Vec::new(),
                                badges: st.badges.clone(),
                                segs: Vec::new(),
                                platform: crate::platforms::Platform::Twitch,
                                msg_id: String::new(),
                                user_id: String::new(),
                            },
                        ));
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => return Ok(Exit::Quit),
            }
        }
        if st.channels.is_empty() {
            let _ = ws.close(None);
            return Ok(Exit::Reconnect);
        }
        if last_ping.elapsed() > Duration::from_secs(240) {
            text(&mut ws, "PING :keepalive".into())?;
            last_ping = Instant::now();
        }
        match ws.read() {
            Ok(Message::Text(t)) => {
                for raw in t.lines() {
                    if raw.starts_with("PING") {
                        text(&mut ws, raw.replacen("PING", "PONG", 1))?;
                        continue;
                    }
                    if raw.contains(" RECONNECT") && !raw.contains("PRIVMSG") {
                        return Ok(Exit::Reconnect);
                    }
                    handle(raw, st, emit);
                    if raw.contains("Login authentication failed")
                        || raw.contains("Improperly formatted auth")
                    {
                        emit(Event::AuthFailed);
                        return Err("Twitch refused the sign-in".into());
                    }
                }
            }
            Ok(Message::Close(_)) => return Err("closed by server".into()),
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(e) => return Err(e.to_string()),
        }
    }
}

fn handle(raw: &str, st: &mut State, emit: &impl Fn(Event)) {
    let Some(p) = parse(raw) else { return };
    match p.cmd {
        "GLOBALUSERSTATE" | "USERSTATE" => {
            if let Some(c) = tag(p.tags, "color").and_then(parse_color) {
                st.color = Some(c);
            }
            if let Some(d) = tag(p.tags, "display-name").filter(|s| !s.is_empty()) {
                st.display = Some(unescape(d));
            }
            if let Some(b) = tag(p.tags, "badges") {
                st.badges = parse_badges(b);
            }
        }
        "ROOMSTATE" => {
            if let (Some(ch), Some(id)) = (channel_of(p.params), tag(p.tags, "room-id")) {
                emit(Event::Room(ch, id.to_string()));
            }
        }
        _ => {
            if let Some((ch, line)) = to_line(&p) {
                match ch {
                    Some(ch) => emit(Event::Line(ch, line)),
                    None => emit(Event::All(line)),
                }
            }
        }
    }
}

struct Parsed<'a> {
    tags: &'a str,
    prefix: &'a str,
    cmd: &'a str,
    params: &'a str,
}

fn parse(raw: &str) -> Option<Parsed<'_>> {
    let (tags, rest) = match raw.strip_prefix('@') {
        Some(r) => r.split_once(' ')?,
        None => ("", raw),
    };
    let (prefix, rest) = match rest.strip_prefix(':') {
        Some(r) => r.split_once(' ')?,
        None => ("", rest),
    };
    let (cmd, params) = rest.split_once(' ').unwrap_or((rest, ""));
    Some(Parsed {
        tags,
        prefix,
        cmd,
        params,
    })
}

fn channel_of(params: &str) -> Option<String> {
    let first = params.split(' ').next()?;
    first.strip_prefix('#').map(|c| c.to_lowercase())
}

fn trailing(params: &str) -> &str {
    if let Some(t) = params.strip_prefix(':') {
        return t;
    }
    params.split_once(" :").map(|x| x.1).unwrap_or("")
}

/// Turns one IRC line into a chat line and the channel it belongs to (None = all tabs).
fn to_line(p: &Parsed<'_>) -> Option<(Option<String>, ChatLine)> {
    let ch = channel_of(p.params);
    match p.cmd {
        "PRIVMSG" => {
            let text = trailing(p.params);
            let user = p.prefix.split('!').next().unwrap_or("");
            let name = tag(p.tags, "display-name")
                .filter(|s| !s.is_empty())
                .map(unescape)
                .unwrap_or_else(|| user.to_string());
            let color = tag(p.tags, "color").and_then(parse_color);
            let (text, action) = match text
                .strip_prefix("\u{1}ACTION ")
                .and_then(|t| t.strip_suffix('\u{1}'))
            {
                Some(t) => (t.to_string(), true),
                None => (text.to_string(), false),
            };
            let emotes = tag(p.tags, "emotes")
                .map(crate::emotes::parse_emote_tag)
                .unwrap_or_default();
            let badges = tag(p.tags, "badges").map(parse_badges).unwrap_or_default();
            Some((
                ch,
                ChatLine::Msg {
                    time: clock(),
                    name,
                    color,
                    text,
                    action,
                    own: false,
                    emotes,
                    badges,
                    segs: Vec::new(),
                    platform: crate::platforms::Platform::Twitch,
                    msg_id: tag(p.tags, "id").unwrap_or("").to_string(),
                    user_id: tag(p.tags, "user-id").unwrap_or("").to_string(),
                },
            ))
        }
        // without the membership capability Twitch only sends our own JOIN
        "JOIN" => {
            let ch = ch?;
            Some((Some(ch.clone()), ChatLine::System(format!("Joined #{ch}"))))
        }
        "NOTICE" => {
            let msg = trailing(p.params).to_string();
            if ch.is_none() && msg.is_empty() {
                return None;
            }
            Some((ch, ChatLine::System(msg)))
        }
        "USERNOTICE" => {
            let msg = tag(p.tags, "system-msg").map(unescape)?;
            let extra = trailing(p.params);
            let msg = if extra.is_empty() {
                msg
            } else {
                format!("{msg} {extra}")
            };
            Some((ch, ChatLine::System(msg)))
        }
        "CLEARCHAT" => {
            let who = trailing(p.params);
            let msg = if who.is_empty() {
                "Chat was cleared by a moderator.".to_string()
            } else {
                match tag(p.tags, "ban-duration") {
                    Some(d) => format!("{who} was timed out for {d}s."),
                    None => format!("{who} was banned."),
                }
            };
            Some((ch, ChatLine::System(msg)))
        }
        "ROOMSTATE" => None,
        "001" => {
            let me = p.params.split(' ').next().unwrap_or("").to_string();
            Some((
                None,
                ChatLine::System(if me.starts_with("justinfan") {
                    "Connected (read only; sign in to send)".into()
                } else {
                    format!("Connected as {me}")
                }),
            ))
        }
        _ => None,
    }
}

fn tag<'a>(tags: &'a str, key: &str) -> Option<&'a str> {
    tags.split(';').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == key).then_some(v)
    })
}

/// "moderator/1,subscriber/12" -> [(moderator, 1), (subscriber, 12)]
fn parse_badges(v: &str) -> Vec<(String, String)> {
    v.split(',')
        .filter_map(|b| b.split_once('/'))
        .map(|(s, ver)| (s.to_string(), ver.to_string()))
        .collect()
}

fn unescape(v: &str) -> String {
    v.replace("\\s", " ")
        .replace("\\:", ";")
        .replace("\\\\", "\\")
}

fn parse_color(hex: &str) -> Option<[u8; 3]> {
    let h = hex.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some([p(0)?, p(2)?, p(4)?])
}

pub fn clock() -> String {
    clock_at(crate::model::now_unix())
}

/// Local HH:MM of a Unix time.
pub fn clock_at(unix: i64) -> String {
    let t = unix + local_offset_seconds();
    let d = t.rem_euclid(86400);
    format!("{:02}:{:02}", d / 3600, d % 3600 / 60)
}

/// Local time offset in seconds, asked once from Windows (UTC elsewhere).
fn local_offset_seconds() -> i64 {
    static OFFSET: std::sync::OnceLock<i64> = std::sync::OnceLock::new();
    *OFFSET.get_or_init(windows_offset)
}

#[cfg(windows)]
fn windows_offset() -> i64 {
    #[repr(C)]
    struct Tzi {
        bias: i32,
        rest: [u8; 168],
    }
    extern "system" {
        fn GetTimeZoneInformation(tzi: *mut Tzi) -> u32;
    }
    let mut tzi = Tzi {
        bias: 0,
        rest: [0; 168],
    };
    // SAFETY: plain Win32 call writing into a correctly sized struct.
    let kind = unsafe { GetTimeZoneInformation(&mut tzi) };
    // rest[164..168] is DaylightBias, used when daylight time is active (kind == 2)
    let daylight = if kind == 2 {
        i32::from_le_bytes([tzi.rest[164], tzi.rest[165], tzi.rest[166], tzi.rest[167]])
    } else {
        0
    };
    -((tzi.bias + daylight) as i64) * 60
}

#[cfg(not(windows))]
fn windows_offset() -> i64 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_privmsg() {
        let raw = "@badge-info=;color=#FF7F50;display-name=SomeOne;emotes= :someone!someone@someone.tmi.twitch.tv PRIVMSG #chan :hello there";
        let p = parse(raw).unwrap();
        match to_line(&p) {
            Some((
                Some(ch),
                ChatLine::Msg {
                    name, color, text, ..
                },
            )) => {
                assert_eq!(ch, "chan");
                assert_eq!(name, "SomeOne");
                assert_eq!(color, Some([0xFF, 0x7F, 0x50]));
                assert_eq!(text, "hello there");
            }
            _ => panic!("not parsed"),
        }
    }

    #[test]
    fn parses_notice() {
        let raw = "@msg-id=msg_followersonly :tmi.twitch.tv NOTICE #chan :This room is in followers-only mode.";
        let p = parse(raw).unwrap();
        assert!(matches!(to_line(&p), Some((Some(_), ChatLine::System(_)))));
    }

    fn wait_for(chat: &Chat, what: impl Fn(&Event) -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            if let Ok(e) = chat.rx.recv_timeout(Duration::from_millis(500)) {
                println!("{e:?}");
                if what(&e) {
                    return true;
                }
            }
        }
        false
    }

    /// Network test: `cargo test -- --ignored joins_anonymously`
    #[test]
    #[ignore]
    fn joins_anonymously() {
        let chat = Chat::start(egui::Context::default());
        chat.send(Cmd::Join("twitch".into()));
        assert!(wait_for(
            &chat,
            |e| matches!(e, Event::Line(_, ChatLine::System(t)) if t.starts_with("Joined"))
        ));
    }

    /// Network test with the Moltorino account: logs in and joins the account's own
    /// channel. Sends nothing. `cargo test -- --ignored signs_in --nocapture`
    #[test]
    #[ignore]
    fn signs_in() {
        let Some(acc) = crate::account::test_account() else {
            println!("set LIVE_STREAMS_TEST_LOGIN to run this test");
            return;
        };
        let chat = Chat::start(egui::Context::default());
        chat.send(Cmd::Auth(Some((acc.login.clone(), acc.token.clone()))));
        chat.send(Cmd::Join(acc.login.clone()));
        assert!(wait_for(
            &chat,
            |e| matches!(e, Event::All(ChatLine::System(t)) if t.starts_with("Connected as"))
        ));
        assert!(wait_for(
            &chat,
            |e| matches!(e, Event::Line(_, ChatLine::System(t)) if t.starts_with("Joined"))
        ));
    }
}
