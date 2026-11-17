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
        /// The message this one answers (Twitch replies).
        reply: Option<Box<Reply>>,
    },
    System(String),
    /// A message AutoMod held for moderators to allow or deny (Chatterino's AutoMod line).
    AutoMod(Box<HeldMessage>),
}

/// A message AutoMod held (EventSub automod.message.hold).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HeldMessage {
    pub time: String,
    pub msg_id: String,
    pub user_id: String,
    pub user: String,
    pub text: String,
    /// Why: AutoMod's category and level, or the blocked term.
    pub reason: String,
    /// Empty while it waits; else "approved", "denied" or "expired" and by whom.
    pub status: String,
}

impl HeldMessage {
    /// One line of text (export, logs, search, copy).
    pub fn summary(&self) -> String {
        let mut s = format!(
            "AutoMod held a message from {} ({}): {}",
            self.user, self.reason, self.text
        );
        if !self.status.is_empty() {
            s.push_str(&format!(" [{}]", self.status));
        }
        s
    }
}

/// What a reply answers (Twitch's reply-parent-* and reply-thread-parent-* tags).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reply {
    /// The answered message's id; replying to it sends this.
    pub parent_id: String,
    pub parent_name: String,
    pub parent_body: String,
    /// The first message of the thread (a reply to a reply belongs to the same thread).
    pub thread_id: String,
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
    /// Whether the signed-in account moderates this channel (USERSTATE: mod=1, or the
    /// broadcaster badge in its own channel); the usercard's mod actions and the split's
    /// moderation mode (sword) need it.
    ModIn(String, bool),
    /// The id Twitch gave a message this account just sent (its USERSTATE after a PRIVMSG):
    /// (channel, message id). Pinning, deleting and replying need it.
    OwnId(String, String),
}

pub enum Cmd {
    Join(String),
    Part(String),
    /// (channel, text, the message it answers when it's a reply)
    Send(String, String, Option<Reply>),
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
        Cmd::Send(ch, _, _) => emit(Event::Line(
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

/// Channels waiting to be joined, sent at most JOIN_BURST per JOIN_WINDOW (Twitch's limit is
/// about 20 per 10 seconds; a little under it leaves room).
#[derive(Default)]
struct JoinQueue {
    waiting: std::collections::VecDeque<String>,
    sent: std::collections::VecDeque<Instant>,
}

const JOIN_BURST: usize = 15;
const JOIN_WINDOW: Duration = Duration::from_secs(11);

impl JoinQueue {
    fn add(&mut self, ch: String) {
        if !self.waiting.contains(&ch) {
            self.waiting.push_back(ch);
        }
    }

    /// Takes a channel out before it was joined; true when it was still waiting.
    fn cancel(&mut self, ch: &str) -> bool {
        let before = self.waiting.len();
        self.waiting.retain(|c| c != ch);
        self.waiting.len() != before
    }

    /// The next channel to join now, if the limit allows one.
    fn next(&mut self, now: Instant) -> Option<String> {
        while self
            .sent
            .front()
            .is_some_and(|t| now.duration_since(*t) >= JOIN_WINDOW)
        {
            self.sent.pop_front();
        }
        if self.sent.len() >= JOIN_BURST {
            return None;
        }
        let ch = self.waiting.pop_front()?;
        self.sent.push_back(now);
        Some(ch)
    }
}

fn text(ws: &mut Ws, line: String) -> Result<(), String> {
    if !line.starts_with("PASS ") {
        debug_irc(">", &line);
    }
    ws.send(Message::Text(line.into()))
        .map_err(|e| e.to_string())
}

/// Troubleshooting: YAPERINO_DEBUG_IRC=<file> writes the chat traffic there (never the
/// PASS line, which holds the sign-in).
fn debug_irc(dir: &str, line: &str) {
    let Ok(path) = std::env::var("YAPERINO_DEBUG_IRC") else {
        return;
    };
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{} {dir} {line}", clock_secs());
    }
}

fn clock_secs() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
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
    // Twitch takes about 20 JOINs per 10 seconds and silently drops the rest: with many
    // tabs open, some chats stayed empty. Joins go out in paced batches instead.
    let mut joins = JoinQueue::default();
    let mut first: Vec<String> = st.channels.iter().cloned().collect();
    first.sort();
    for ch in first {
        joins.add(ch);
    }
    let mut last_ping = Instant::now();
    loop {
        // commands from the window
        loop {
            match cmds.try_recv() {
                Ok(Cmd::Join(ch)) => {
                    if st.channels.insert(ch.clone()) {
                        joins.add(ch);
                    }
                }
                Ok(Cmd::Part(ch)) => {
                    // not joined yet: just never join it
                    if st.channels.remove(&ch) && !joins.cancel(&ch) {
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
                Ok(Cmd::Send(ch, msg, reply)) => {
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
                        // a reply carries the answered message's id (Twitch's reply tag)
                        let raw = match &reply {
                            Some(r) => {
                                format!("@reply-parent-msg-id={} PRIVMSG #{ch} :{msg}", r.parent_id)
                            }
                            None => format!("PRIVMSG #{ch} :{msg}"),
                        };
                        text(&mut ws, raw)?;
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
                                reply: reply.map(Box::new),
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
        while let Some(ch) = joins.next(Instant::now()) {
            text(&mut ws, format!("JOIN #{ch}"))?;
        }
        if last_ping.elapsed() > Duration::from_secs(240) {
            text(&mut ws, "PING :keepalive".into())?;
            last_ping = Instant::now();
        }
        match ws.read() {
            Ok(Message::Text(t)) => {
                for raw in t.lines() {
                    debug_irc("<", raw);
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
            // USERSTATE is per channel: are we a moderator (or the broadcaster) there?
            if p.cmd == "USERSTATE" {
                if let Some(ch) = channel_of(p.params) {
                    emit(Event::ModIn(ch.clone(), moderates(p.tags)));
                    // after a message we sent, USERSTATE carries its id
                    if let Some(id) = tag(p.tags, "id").filter(|s| !s.is_empty()) {
                        emit(Event::OwnId(ch, id.to_string()));
                    }
                }
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

/// The message a PRIVMSG answers, from its reply-parent-* tags.
fn reply_of(tags: &str) -> Option<Reply> {
    let parent_id = tag(tags, "reply-parent-msg-id").filter(|s| !s.is_empty())?;
    let parent_name = tag(tags, "reply-parent-display-name")
        .filter(|s| !s.is_empty())
        .or_else(|| tag(tags, "reply-parent-user-login"))
        .map(unescape)
        .unwrap_or_default();
    Some(Reply {
        parent_id: parent_id.to_string(),
        parent_name,
        parent_body: tag(tags, "reply-parent-msg-body")
            .map(unescape)
            .unwrap_or_default(),
        thread_id: tag(tags, "reply-thread-parent-msg-id")
            .filter(|s| !s.is_empty())
            .unwrap_or(parent_id)
            .to_string(),
    })
}

/// How many characters "@parent " takes at the start of a reply (by login or display name).
fn reply_prefix_len(text: &str, login: &str, display: &str) -> Option<usize> {
    let word = text.strip_prefix('@')?.split(' ').next()?;
    let matches = (!login.is_empty() && word.eq_ignore_ascii_case(login))
        || (!display.is_empty() && word.eq_ignore_ascii_case(display));
    matches.then(|| {
        1 + word.chars().count()
            + usize::from(text.chars().nth(1 + word.chars().count()) == Some(' '))
    })
}

/// A USERSTATE's tags: moderator there (mod=1), or its broadcaster.
fn moderates(tags: &str) -> bool {
    tag(tags, "mod") == Some("1")
        || tag(tags, "badges")
            .map(parse_badges)
            .is_some_and(|b| b.iter().any(|(set, _)| set == "broadcaster"))
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
            let mut emotes = tag(p.tags, "emotes")
                .map(crate::emotes::parse_emote_tag)
                .unwrap_or_default();
            let badges = tag(p.tags, "badges").map(parse_badges).unwrap_or_default();
            let reply = reply_of(p.tags);
            // a reply starts with "@parent " (Twitch adds it); the reply line above shows who,
            // so it goes, like in Chatterino (and the emote positions move with the text)
            let mut text = text;
            if let Some(r) = &reply {
                let login = tag(p.tags, "reply-parent-user-login").unwrap_or("");
                if let Some(cut) = reply_prefix_len(&text, login, &r.parent_name) {
                    text = text.chars().skip(cut).collect();
                    emotes = emotes
                        .into_iter()
                        .filter(|(a, _, _)| *a >= cut)
                        .map(|(a, b, id)| (a - cut, b - cut, id))
                        .collect();
                }
            }
            Some((
                ch,
                ChatLine::Msg {
                    time: message_time(p.tags),
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
                    reply: reply.map(Box::new),
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

/// The address of the chat history service Chatterino uses by default.
pub const HISTORY_URL: &str = "https://recent-messages.robotty.de/api/v2/recent-messages/";

/// Chat history: the raw Twitch chat lines recent-messages.robotty.de returns, as chat lines
/// (messages, sub / raid notices, timeouts), oldest first.
pub fn history_lines(raw: &[String]) -> Vec<ChatLine> {
    raw.iter()
        .filter_map(|r| parse(r))
        .filter(|p| matches!(p.cmd, "PRIVMSG" | "USERNOTICE" | "CLEARCHAT"))
        .filter_map(|p| to_line(&p).map(|(_, l)| l))
        .collect()
}

/// A message's time: now, or for chat history (historical=1) when it was sent.
fn message_time(tags: &str) -> String {
    let sent = (tag(tags, "historical") == Some("1"))
        .then(|| tag(tags, "tmi-sent-ts"))
        .flatten()
        .and_then(|t| t.parse::<i64>().ok());
    match sent {
        Some(ms) => clock_at(ms / 1000),
        None => clock(),
    }
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

/// Linux and macOS: the local time zone's offset from UTC in seconds.
#[cfg(not(windows))]
fn windows_offset() -> i64 {
    chrono::Local::now().offset().local_minus_utc() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reply_tags_cut_the_mention_and_move_the_emotes() {
        let raw = concat!(
            "@badges=;color=;display-name=Bob;emotes=25:7-11;id=m2;",
            "reply-parent-display-name=Alice;reply-parent-msg-body=hi;",
            "reply-parent-msg-id=m1;reply-parent-user-login=alice;",
            "reply-thread-parent-msg-id=m0;user-id=2 ",
            ":bob!bob@bob.tmi.twitch.tv PRIVMSG #chan :@alice Kappa yes"
        );
        let p = parse(raw).unwrap();
        let (ch, line) = to_line(&p).unwrap();
        assert_eq!(ch.as_deref(), Some("chan"));
        let ChatLine::Msg {
            text,
            emotes,
            reply,
            ..
        } = line
        else {
            panic!("not a message");
        };
        assert_eq!(text, "Kappa yes");
        assert_eq!(emotes, vec![(0, 4, "25".to_string())]);
        let r = reply.unwrap();
        assert_eq!(
            (
                r.parent_id.as_str(),
                r.parent_name.as_str(),
                r.parent_body.as_str(),
                r.thread_id.as_str()
            ),
            ("m1", "Alice", "hi", "m0")
        );
    }

    #[test]
    fn own_messages_get_their_id_from_userstate() {
        let mut st = State {
            channels: HashSet::new(),
            auth: None,
            color: None,
            badges: Vec::new(),
            display: None,
        };
        let got = std::cell::RefCell::new(Vec::new());
        let emit = |e: Event| got.borrow_mut().push(e);
        handle(
            "@badges=broadcaster/1;color=;display-name=Me;id=abc-123;mod=0 :tmi.twitch.tv USERSTATE #me",
            &mut st,
            &emit,
        );
        // joining: no id
        handle(
            "@badges=broadcaster/1;color=;display-name=Me;mod=0 :tmi.twitch.tv USERSTATE #me",
            &mut st,
            &emit,
        );
        let ids: Vec<(String, String)> = got
            .borrow()
            .iter()
            .filter_map(|e| match e {
                Event::OwnId(c, i) => Some((c.clone(), i.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(ids, [("me".to_string(), "abc-123".to_string())]);
    }

    #[test]
    fn joins_are_paced_under_twitchs_limit() {
        let mut q = JoinQueue::default();
        for i in 0..27 {
            q.add(format!("c{i}"));
        }
        q.add("c0".into()); // already waiting
        let t0 = Instant::now();
        let first: Vec<String> = std::iter::from_fn(|| q.next(t0)).collect();
        assert_eq!(first.len(), JOIN_BURST);
        // a chat closed before its turn is never joined
        assert!(q.cancel("c20"));
        assert!(!q.cancel("c0"));
        assert_eq!(q.next(t0 + Duration::from_secs(5)), None);
        let rest: Vec<String> = std::iter::from_fn(|| q.next(t0 + JOIN_WINDOW)).collect();
        assert_eq!(rest.len(), 27 - JOIN_BURST - 1);
        assert!(!rest.contains(&"c20".to_string()));
    }

    #[test]
    fn reply_prefix() {
        assert_eq!(reply_prefix_len("@alice hi", "alice", "Alice"), Some(7));
        assert_eq!(reply_prefix_len("@Alice hi", "alice", ""), Some(7));
        assert_eq!(reply_prefix_len("@bob hi", "alice", "Alice"), None);
        assert_eq!(reply_prefix_len("hi @alice", "alice", "Alice"), None);
    }

    #[test]
    fn chat_history_lines_keep_their_time() {
        let raw = vec![
            concat!(
                "@badges=;display-name=Old;historical=1;id=h1;tmi-sent-ts=1700000000000;",
                "user-id=9 :old!old@old.tmi.twitch.tv PRIVMSG #chan :from before"
            )
            .to_string(),
            ":old!old@old.tmi.twitch.tv JOIN #chan".to_string(),
            "not a chat line".to_string(),
        ];
        let lines = history_lines(&raw);
        assert_eq!(lines.len(), 1);
        let ChatLine::Msg {
            name, text, time, ..
        } = &lines[0]
        else {
            panic!("not a message");
        };
        assert_eq!((name.as_str(), text.as_str()), ("Old", "from before"));
        assert_eq!(*time, clock_at(1_700_000_000));
    }

    #[test]
    fn moderator_in_a_channel() {
        assert!(moderates(
            "badge-info=;badges=moderator/1;color=;mod=1;subscriber=0"
        ));
        // your own channel: the broadcaster badge, mod=0
        assert!(moderates("badges=broadcaster/1,premium/1;mod=0"));
        assert!(!moderates("badges=subscriber/12;mod=0"));
        assert!(!moderates("badges=;mod=0"));
    }

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

    /// Network test: other people's messages arrive (read only, in a busy chat).
    /// `cargo test -- --ignored receives_messages` (YAP_TEST_CHANNEL to pick the channel)
    #[test]
    #[ignore]
    fn receives_messages() {
        let ch = std::env::var("YAP_TEST_CHANNEL").unwrap_or_else(|_| "xqc".into());
        let chat = Chat::start(egui::Context::default());
        chat.send(Cmd::Join(ch));
        assert!(wait_for(&chat, |e| matches!(
            e,
            Event::Line(_, ChatLine::Msg { own: false, .. })
        )));
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
