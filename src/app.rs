//! The Live Streams window (main window) and the Chats window with tabs.

use crate::account::{self, Account, SavedDevice};
use crate::chat::{self, ChatLine, Cmd, Event};
use crate::emotes::{self, Store};
use crate::platforms::{self, Links, Platform};

#[path = "backup.rs"]
mod backup;
#[path = "chat_main.rs"]
mod chat_main;
#[path = "features.rs"]
mod features;
#[path = "search.rs"]
mod search;
#[path = "settings_ui.rs"]
mod settings_ui;
#[path = "usercard.rs"]
mod usercard;
use crate::browser;
use crate::fetch::{self, FetchResult, Job};
use crate::model::{self, SortMode, Stream};
use egui::{Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use settings_ui::{Page, SettingsUi};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ---- look (measured from the person's picture; see MECHANICS.md) ----
pub const BG: Color32 = Color32::from_rgb(59, 59, 59);
pub const RULE: Color32 = Color32::from_rgb(32, 32, 32);
pub const TEXT: Color32 = Color32::WHITE;
const HOVER: Color32 = Color32::from_rgb(66, 66, 66);
const SELECTED: Color32 = Color32::from_rgb(76, 76, 80);
pub const ROW_H: f32 = 60.0;
const RULE_Y: f32 = 13.0; // rule (and centre of the name) below the top of a row
const TITLE_Y: f32 = 32.0; // centre of the title line
const LINE_H: f32 = 15.0; // title/game lines are 15 px apart (picture: 86 -> 101)
/// Rule of a stream that went live or changed title/game in the last 3 minutes, and of the
/// selected stream (Chatty: TITLE_NEW / TITLE_SELECTED borders; on this dark background the
/// new rule is drawn light so it stands out).
const RULE_NEW: Color32 = Color32::from_rgb(200, 200, 200);
const RULE_SELECTED: Color32 = Color32::from_rgb(120, 120, 120);
const LEFT: f32 = 10.0;
/// Height and colour of a platform section heading.
const SECTION_H: f32 = 28.0;
const SECTION_BG: Color32 = Color32::from_rgb(46, 46, 46);
const RIGHT: f32 = 6.0;
const NAME_GAP: f32 = 4.0;
pub const TEXT_SIZE: f32 = 12.5;

/// Default time between live checks (changeable in Settings > General).
pub const REFRESH_SECS: u64 = 120;
const MAX_CHAT_LINES: usize = 600;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    pub sort: SortMode,
    /// Channels added in this window.
    pub extra: Vec<String>,
    /// Which Twitch account to use for follows and sending messages.
    pub account: AccountMode,
    /// Older single sign-in (moved into 	witch_accounts on start).
    pub device: SavedDevice,
    /// This app's own Twitch accounts (Chatterino/Moltorino-style account list) and the current one.
    pub twitch_accounts: Vec<SavedDevice>,
    pub current_twitch: String,
    /// The Client ID of the person's Twitch app, for device login.
    pub twitch_client_id: String,
    /// Favourite channels (logins) and games, as in Chatty.
    pub fav_channels: HashSet<String>,
    pub fav_games: HashSet<String>,
    /// Chatty's "Favorites first" sort option and "Show favorites only" filter.
    pub fav_first: bool,
    pub favs_only: bool,
    /// Each Twitch channel's Kick / YouTube / TikTok channels.
    pub links: HashMap<String, Links>,
    /// Use the Kick account Moltorino is signed in with, for sending to Kick.
    pub use_moltorino_kick: bool,
    /// Which platform the message box sends to.
    pub send_platform: Platform,
    /// Seconds between live checks.
    pub refresh_secs: u64,
    pub chat_font: f32,
    pub timestamps: bool,
    /// Reopen chat tabs at start, and the tabs that were open: (tab key, name).
    pub restore_tabs: bool,
    pub open_tabs: Vec<(String, String)>,
    pub streamlink: crate::streamlink::StreamlinkSettings,
    /// This app's own YouTube (Google) sign-in.
    pub youtube: crate::youtube_auth::YouTubeSaved,
    /// This app's own Kick sign-in (the person's Kick app).
    pub kick: crate::kick_auth::KickSaved,
    /// Emotes (Settings > Emotes), like Chatterino's emote settings.
    pub emote_scale: f32,
    pub show_emote_images: bool,
    pub emote_tooltips: bool,
    pub show_badges: bool,
    /// FFZ, BTTV, 7TV switched off.
    pub emotes_off: [bool; 3],
    /// Recently used emotes for the emote menu: (name, url, provider label).
    pub recent_emotes: Vec<(String, String, String)>,
    pub recent_limit: usize,
    pub close_menu_on_pick: bool,
    /// Chat look (Settings > Chat).
    pub alternate_bg: bool,
    pub separators: bool,
    pub highlight_mentions: bool,
    /// One section per platform in the live list.
    pub group_by_platform: bool,
    /// Split header (Chatterino's header settings): viewers, uptime, title, game.
    pub header_viewers: bool,
    pub header_uptime: bool,
    pub header_title: bool,
    pub header_game: bool,
    /// Tabs with their splits: [(key, name)] per tab.
    pub open_pages: Vec<Vec<(String, String)>>,
    /// Open the Live Streams window when the app starts.
    pub live_on_start: bool,
    /// Tabs with their splits and names (replaces open_pages).
    pub tabs: Vec<TabPage>,
    /// Chatterino pages: Nicknames (user -> name), Commands, Highlights, Ignores, Live Notifications.
    pub nicknames: Vec<(String, String)>,
    pub commands: Vec<CustomCommand>,
    pub highlight_phrases: Vec<HighlightRule>,
    pub highlight_users: Vec<HighlightRule>,
    pub ignore_phrases: Vec<String>,
    pub ignore_users: Vec<String>,
    pub notify_channels: Vec<String>,
    /// Also notify for every followed / listed channel that goes live.
    pub notify_all: bool,
    pub notify_flash: bool,
    /// Look up every followed / listed / joined Twitch channel on Kick, YouTube and TikTok
    /// by its name, and list it there when live (even without following it there).
    pub match_names: bool,
    /// What those lookups found: Twitch login -> its other channels.
    pub matched: HashMap<String, Links>,
    /// The follow list from the last check (offline channels too), for name matching.
    pub follows: Vec<String>,
    /// Settings > Customization (chatterino7 / Moltorino appearance).
    pub appearance: crate::style::Appearance,
    /// Chatterino's message settings: a platform badge before each message, bold and
    /// coloured user names, how many messages a chat keeps.
    pub platform_badges: bool,
    pub bold_names: bool,
    pub user_colors: bool,
    pub message_limit: usize,
    /// Chatterino's completion: suggestions while typing :emote / @user, and Tab completion.
    pub completion_popup: bool,
    pub tab_completion: bool,
    /// The message length counter and the live dot on tabs.
    pub input_counter: bool,
    pub tab_live_dot: bool,
    /// Highlights > Badges and > Blacklisted Users; play the sound while Yaperino is focused.
    pub highlight_badges: Vec<HighlightRule>,
    pub highlight_blacklist: Vec<String>,
    pub highlight_sound_focused: bool,
    /// Ignores > Messages (replaces the plain ignore_phrases, which are moved in once).
    pub ignore_rules: Vec<IgnoreRule>,
    /// Filters, the filters each split uses (tab key -> filter names), and whether one's own
    /// messages are always shown.
    pub filters: Vec<FilterDef>,
    pub split_filters: HashMap<String, Vec<String>>,
    pub filters_skip_own: bool,
    /// Moderation > Logs: chat logs as text files per channel and day.
    pub logging: bool,
    /// Settings Manager: a backup of the settings once a day (the last 10 are kept).
    pub auto_backup: bool,
    pub log_dir: String,
    pub log_seconds: bool,
    pub log_only_listed: bool,
    pub log_channels: Vec<String>,
    /// Live Notifications > Options.
    pub notify_sound_selected: bool,
    pub notify_sound_all: bool,
    pub notify_suppress_startup: bool,
    /// General > Interface: tab visibility, always on top, start with Windows.
    pub tab_live_only: bool,
    /// Hide all tabs (Chatterino's Ctrl+U "Toggle tab visibility").
    pub tabs_hidden: bool,
    /// Chatterino's "Lock tab layout": no tab can be moved or closed.
    pub tabs_locked: bool,
    /// Moltorino: no padlock or close-button space on tabs while the layout is locked.
    pub tabs_hide_lock: bool,
    /// Moltorino: thinner tab lines and tighter tab spacing.
    pub tabs_compact: bool,
    /// Collapsed tab groups (their tabs hidden but the shown one).
    pub collapsed_groups: Vec<String>,
    /// Moltorino /hide: users whose messages are hidden here (not blocked, not banned).
    pub hidden_users: Vec<String>,
    /// Moltorino message source display: a platform-coloured background behind messages.
    pub source_background: bool,
    /// Collapse a user's identical message sent again right after into one line with ×N.
    pub collapse_repeats: bool,
    /// Chatterino: load the recent chat history when a Twitch chat opens
    /// (recent-messages.robotty.de).
    pub load_history: bool,
    /// Settings > Hotkeys: action id -> keys, where the person changed them (empty = none).
    pub hotkeys: HashMap<String, String>,
    /// Moderation > Moderation buttons / User Timeout Buttons.
    pub mod_buttons: bool,
    pub timeout_buttons: Vec<String>,
    /// Chatterino's moderation mode (the sword in a split's header): Twitch channels that show
    /// the moderation buttons in front of every message.
    pub mod_mode: Vec<String>,
    /// Live Notifications > Show notification (Windows), and what clicking it does.
    pub notify_toast: bool,
    pub notify_click: String,
    /// External tools > Image Uploader (Chatterino's): paste an image into a message box.
    pub uploader_enabled: bool,
    pub uploader_url: String,
    pub uploader_field: String,
    pub uploader_headers: String,
    pub uploader_link: String,
    pub uploader_delete: String,
    pub always_on_top: bool,
    /// Moltorino "Hide to tray": closing the window hides it to the tray (Windows).
    pub hide_to_tray: bool,
    pub start_with_windows: bool,
    /// External tools > Custom stream player: a URI scheme put before the stream's address.
    pub custom_player: String,
    /// Plugins: on/off for all, and the ones switched on.
    pub plugins_enabled: bool,
    pub enabled_plugins: Vec<String>,
    /// Emote window > Favourite: (name, url, provider label).
    pub emote_favourites: Vec<(String, String, String)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AccountMode {
    /// From older versions (read Moltorino's sign-in); now treated as Device.
    Moltorino,
    /// This app's own Twitch accounts (the current one).
    Device,
    /// No account: read-only chat, live list from the channel list only.
    Off,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            sort: SortMode::Viewers,
            extra: Vec::new(),
            account: AccountMode::Device,
            device: SavedDevice::default(),
            twitch_accounts: Vec::new(),
            current_twitch: String::new(),
            twitch_client_id: String::new(),
            fav_channels: HashSet::new(),
            fav_games: HashSet::new(),
            fav_first: false,
            favs_only: false,
            links: HashMap::new(),
            use_moltorino_kick: true,
            send_platform: Platform::Twitch,
            refresh_secs: REFRESH_SECS,
            chat_font: 13.0,
            timestamps: true,
            restore_tabs: true,
            open_tabs: Vec::new(),
            streamlink: Default::default(),
            youtube: Default::default(),
            kick: Default::default(),
            emote_scale: 1.0,
            show_emote_images: true,
            emote_tooltips: true,
            show_badges: true,
            emotes_off: [false; 3],
            recent_emotes: Vec::new(),
            recent_limit: 30,
            close_menu_on_pick: false,
            alternate_bg: false,
            separators: false,
            highlight_mentions: true,
            group_by_platform: true,
            header_viewers: true,
            header_uptime: true,
            header_title: true,
            header_game: true,
            open_pages: Vec::new(),
            live_on_start: false,
            tabs: Vec::new(),
            nicknames: Vec::new(),
            commands: Vec::new(),
            highlight_phrases: Vec::new(),
            highlight_users: Vec::new(),
            ignore_phrases: Vec::new(),
            ignore_users: Vec::new(),
            notify_channels: Vec::new(),
            notify_all: false,
            notify_flash: true,
            match_names: true,
            matched: HashMap::new(),
            follows: Vec::new(),
            appearance: Default::default(),
            platform_badges: true,
            bold_names: true,
            user_colors: true,
            message_limit: MAX_CHAT_LINES,
            completion_popup: true,
            tab_completion: true,
            input_counter: true,
            tab_live_dot: true,
            highlight_badges: Vec::new(),
            highlight_blacklist: Vec::new(),
            highlight_sound_focused: false,
            ignore_rules: Vec::new(),
            filters: Vec::new(),
            split_filters: HashMap::new(),
            filters_skip_own: true,
            logging: false,
            auto_backup: true,
            log_dir: String::new(),
            log_seconds: true,
            log_only_listed: false,
            log_channels: Vec::new(),
            notify_sound_selected: false,
            notify_sound_all: false,
            notify_suppress_startup: true,
            tab_live_only: false,
            tabs_hidden: false,
            tabs_locked: false,
            tabs_hide_lock: false,
            tabs_compact: false,
            collapsed_groups: Vec::new(),
            hidden_users: Vec::new(),
            source_background: false,
            collapse_repeats: false,
            load_history: true,
            hotkeys: HashMap::new(),
            mod_buttons: false,
            mod_mode: Vec::new(),
            notify_toast: false,
            notify_click: "Open chat tab".into(),
            uploader_enabled: false,
            uploader_url: "https://kappa.lol/api/upload".into(),
            uploader_field: "file".into(),
            uploader_headers: String::new(),
            uploader_link: "{link}".into(),
            uploader_delete: "{delete}".into(),
            timeout_buttons: ["1s", "30s", "1m", "5m", "30m", "1h", "1d", "1w"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            always_on_top: false,
            hide_to_tray: false,
            start_with_windows: false,
            custom_player: String::new(),
            plugins_enabled: false,
            enabled_plugins: Vec::new(),
            emote_favourites: Vec::new(),
        }
    }
}

struct ChatTab {
    /// The tab's key: a Twitch login, or kick:slug / youtube:@handle / 	iktok:user.
    login: String,
    display: String,
    lines: VecDeque<ChatLine>,
    input: String,
    /// The Twitch channel joined for this tab (its own login, or one found for it).
    twitch: Option<String>,
    /// New messages / a mention since it was last looked at (tab line colours).
    unread: bool,
    mentioned: bool,
    /// Ctrl+F search text (None = search closed).
    search: Option<String>,
    /// Sent messages for Up/Down in the input (Chatterino's previousMessage/nextMessage).
    history: Vec<String>,
    history_pos: usize,
    /// Completion (Chatterino): the selected suggestion, the word whose suggestions were
    /// closed with Escape, and Tab cycling (matches, position, last inserted word).
    ac_sel: usize,
    ac_dismissed: Option<String>,
    tab_cycle: Option<(Vec<String>, usize, String)>,
    /// An image pasted into the message box, waiting for "Upload" (Image Uploader).
    pending_image: Option<Arc<egui::ColorImage>>,
    /// The message the next Twitch message answers (message menu > Reply).
    reply_to: Option<chat::Reply>,
}

/// A tab of the main window (or of a pop-out window): its splits and an optional name.
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct TabPage {
    pub splits: Vec<String>,
    /// Set with "Rename tab"; empty = the channel names.
    pub title: String,
    /// "Lock tab": it can't be closed or moved.
    pub locked: bool,
    /// Moltorino custom tab colour (a line on the tab).
    pub color: Option<[u8; 3]>,
    /// Moltorino "Always visible": shown even when only live tabs are shown.
    pub always_visible: bool,
    /// Moltorino tab group (tab menu > Move to group); empty = none.
    pub group: String,
}

impl TabPage {
    pub fn new(key: &str) -> Self {
        Self {
            splits: vec![key.to_string()],
            ..Default::default()
        }
    }
}

/// A highlight rule (Chatterino's Highlights page): phrase or user, and a colour.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct HighlightRule {
    pub pattern: String,
    pub color: [u8; 3],
    pub enabled: bool,
    /// Chatterino's columns: Enable regex, Case-sensitive, Flash taskbar, Play sound.
    pub regex: bool,
    pub case_sensitive: bool,
    pub flash: bool,
    pub sound: bool,
}

impl Default for HighlightRule {
    fn default() -> Self {
        Self {
            pattern: String::new(),
            color: [0x7f, 0x3f, 0x49],
            enabled: true,
            regex: false,
            case_sensitive: false,
            flash: false,
            sound: false,
        }
    }
}

/// An ignore rule (Chatterino's Ignores > Messages): Pattern, Regex, Case-sensitive, Block,
/// Replacement.
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct IgnoreRule {
    pub pattern: String,
    pub regex: bool,
    pub case_sensitive: bool,
    /// Drop the message (otherwise what matched is replaced).
    pub block: bool,
    pub replacement: String,
}

/// A named filter (Chatterino's Filters page); splits pick filters in their menu.
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct FilterDef {
    pub name: String,
    pub filter: String,
}

/// A custom command (Chatterino's Commands page): /trigger -> text with {1}, {1+}, {channel}.
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct CustomCommand {
    pub trigger: String,
    pub text: String,
}

/// A tab key -> (its first platform, the channel name there).
fn tab_primary(key: &str) -> (Platform, String) {
    for (prefix, p) in [
        ("kick:", Platform::Kick),
        ("youtube:", Platform::YouTube),
        ("tiktok:", Platform::TikTok),
    ] {
        if let Some(rest) = key.strip_prefix(prefix) {
            return (p, rest.to_string());
        }
    }
    (Platform::Twitch, key.to_string())
}

/// The order of the platform sections in the live list.
fn platform_order(p: Platform) -> usize {
    Platform::ALL.iter().position(|x| *x == p).unwrap_or(0)
}

fn tab_key(p: Platform, name: &str) -> String {
    let n = name.trim();
    match p {
        Platform::Twitch => n.trim_start_matches('#').to_lowercase(),
        Platform::Kick => format!("kick:{}", n.to_lowercase()),
        Platform::YouTube => format!(
            "youtube:{}",
            if n.starts_with("UC") || n.starts_with('@') {
                n.to_string()
            } else {
                format!("@{n}")
            }
        ),
        Platform::TikTok => format!("tiktok:{}", n.trim_start_matches('@')),
    }
}

/// Results from the account loader and the device sign-in, which run off the UI thread.
enum AccountMsg {
    Loaded(Result<(Account, Option<SavedDevice>), String>),
    DeviceCode {
        device_code: String,
        user_code: String,
        uri: String,
        interval: u64,
    },
    DeviceDone(Result<SavedDevice, String>),
}

#[derive(Default)]
struct AccountDialog {
    /// "Paste login info" text and the advanced form (username, user ID, client ID, token).
    paste: String,
    adv: [String; 4],
    /// (user code, page) while waiting for the person to approve.
    waiting: Option<(String, String)>,
    message: String,
}

pub struct Shot {
    pub path: PathBuf,
    frames: u32,
    waited: u32,
    requested: bool,
    image: Arc<Mutex<Option<Arc<egui::ColorImage>>>>,
    main_saved: bool,
    /// Test-only (LIVE_STREAMS_SHOT_CHATS): open two sample chat tabs and draw the Chats
    /// window inside the main window, so the one picture shows it too.
    chats: bool,
}

impl Shot {
    pub fn new(path: PathBuf) -> Self {
        let chats = std::env::var("LIVE_STREAMS_SHOT_CHATS").is_ok();
        Self {
            path,
            frames: 0,
            waited: 0,
            requested: false,
            image: Arc::new(Mutex::new(None)),
            main_saved: false,
            chats,
        }
    }
}

pub struct LiveApp {
    settings: Settings,
    streams: Vec<Stream>,
    list_len: usize,
    selected: Option<String>,
    worker: Option<(Sender<Job>, Receiver<FetchResult>)>,
    checking: bool,
    last_check: Option<Instant>,
    first_done: bool,
    /// The last result was the early, Twitch-only one of the first check.
    startup_partial: bool,
    error: Option<String>,
    note: Option<(String, Instant)>,
    chats: Vec<ChatTab>,
    active_chat: usize,
    chat_open: bool,
    add_text: Option<String>,
    last_title: String,
    shot: Option<Shot>,
    demo: bool,
    chat: Option<chat::Chat>,
    chat_name: Option<String>,
    /// Twitch channels the signed-in account moderates (or owns), from the chat (USERSTATE).
    mod_in: HashSet<String>,
    /// The open usercard (Chatterino's user popup), if any.
    usercard: Option<usercard::UserCard>,
    /// The open reply thread: (tab key, the thread's first message id).
    thread: Option<(String, String)>,
    /// Search across all chats (Ctrl+Shift+F), when open.
    global_search: Option<search::GlobalSearch>,
    /// Hide to tray: the main window's handle and the tray icon (once used).
    hwnd: Option<isize>,
    tray: Option<crate::tray::Tray>,
    account: Option<Account>,
    account_status: String,
    account_loading: bool,
    last_account_load: Option<Instant>,
    followed_live: Option<usize>,
    acc_tx: Sender<AccountMsg>,
    acc_rx: Receiver<AccountMsg>,
    acc_dialog: AccountDialog,
    ctx: egui::Context,
    /// Last known (title, game, status change time) per live channel, to tell when it changed.
    seen: HashMap<String, (String, String, i64)>,
    /// Streams that went offline (Chatty's "Offline/Left Streams"), newest first, with the time.
    removed: Vec<(Stream, i64)>,
    show_removed: bool,
    /// Channel shown in the Info window.
    info: Option<String>,
    /// 7TV / BTTV / FFZ emotes and Twitch badges (chatterino7's providers).
    emotes: Store,
    emotes_tx: Sender<EmoteMsg>,
    emotes_rx: Receiver<EmoteMsg>,
    global_emotes_asked: bool,
    global_badges_asked: bool,
    /// Twitch id per channel login (from ROOMSTATE).
    rooms: HashMap<String, String>,
    /// Kick and YouTube chat readers per tab (group) and platform.
    readers: HashMap<(String, Platform), Box<dyn std::any::Any>>,
    other_tx: Sender<Event>,
    other_rx: Receiver<Event>,
    /// Kick ids per group: (broadcaster user id, chatroom id); YouTube live video per group.
    kick_ids: HashMap<String, (u64, u64, u64)>,
    yt_videos: HashMap<String, String>,
    /// Results of finding a channel's other platforms, and of Kick sends.
    side_tx: Sender<SideMsg>,
    side_rx: Receiver<SideMsg>,
    /// Moltorino's Kick sign-in: (username, token) or why not.
    kick_account: Result<(String, String), String>,
    /// The "Platforms" window: (group, links being edited).
    links_edit: Option<(String, Links)>,
    settings_ui: SettingsUi,
    /// Streams picked with Ctrl+click (to open several chat tabs at once).
    multi: HashSet<String>,
    /// The "Open channel" window: platform and name being typed.
    open_dialog: Option<(Platform, String)>,
    /// The emote menu (Chatterino's emote popup) and its search text.
    emote_menu: bool,
    emote_search: String,
    /// Main window tabs (splits + name), and the shown tab.
    pages: Vec<TabPage>,
    active_page: usize,
    /// Pop-out windows (Ctrl+N / Ctrl+Shift+N): (window id, its tab).
    popups: Vec<(u64, TabPage)>,
    next_popup: u64,
    /// Closed splits, newest last (Ctrl+G reopens).
    closed_splits: Vec<(String, String)>,
    /// "Rename tab" in progress: (tab, text).
    renaming: Option<(usize, String)>,
    /// Ctrl+R: the split whose channel is being changed.
    change_channel: Option<String>,
    /// The Live Streams window (Alt+L) is open.
    live_open: bool,
    /// The next opened channel becomes a split in the current tab (Ctrl+T).
    open_split_next: bool,
    /// Name matching: Twitch channels (follows, list, joined) still to look up on Kick,
    /// YouTube and TikTok, and how many lookups run now.
    match_queue: VecDeque<String>,
    match_queued: HashSet<String>,
    matching: usize,
    /// The appearance last applied (applied again when Settings change it).
    applied_look: Option<crate::style::Appearance>,
    /// The daily settings backup was looked at (once per start).
    backup_checked: bool,
    /// Whether the Windows frame was last switched off (own title bar).
    applied_frame: Option<bool>,
    /// The chatters list (Chatterino's viewer list): the split it is for, and its search.
    chatters: Option<(String, String)>,
    /// Plugins (Settings > Plugins): the running ones and the folder's list.
    plugins: crate::plugins::Plugins,
    plugin_list: Vec<crate::plugins::PluginInfo>,
    /// Highlights, ignores and filters compiled from Settings (rebuilt when they change).
    matchers: Arc<crate::rules::Matchers>,
    matchers_key: String,
    /// Always on top, as last applied.
    applied_top: Option<bool>,
    /// The emote window's tab (Favourite, Subs, Channel, Global, Emojis).
    emote_tab: usize,
    /// A tab being dragged to a new place (its index).
    tab_drag: Option<usize>,
    /// Reopening last time's tabs: channels never fill an empty tab then.
    restoring: bool,
    /// Ctrl+T in a pop-out window: the next split goes into that window.
    split_target: Option<u64>,
}

/// Lookups by name at once (each asks Twitch, Kick, YouTube and TikTok).
const MATCH_AT_ONCE: usize = 2;

/// Every Twitch channel the live list knows: joined and added channels, the follow
/// list, and every channel joined in a tab on any platform (its Twitch channel), without
/// repeats and in that order.
pub fn known_channels(
    list: &[String],
    follows: &[String],
    links: &HashMap<String, Links>,
) -> Vec<String> {
    let mut joined: Vec<String> = links
        .iter()
        .filter_map(|(group, l)| match tab_primary(group) {
            (Platform::Twitch, name) => Some(name),
            _ => (!l.twitch.is_empty()).then(|| l.twitch.clone()),
        })
        .filter_map(|n| fetch::valid_login(&n))
        .collect();
    joined.sort();
    let mut seen = HashSet::new();
    list.iter()
        .chain(follows.iter())
        .cloned()
        .chain(joined)
        .filter(|n| seen.insert(n.clone()))
        .collect()
}

/// Background results that aren't live checks.
enum SideMsg {
    /// A followed / listed channel's other platforms, found by name (no tab open).
    Matched(String, Links),
    /// A live notification was clicked: (group, name).
    ToastClicked(String, String),
    /// An image upload finished: (group, image link and deletion link, or why not).
    Uploaded(String, Result<(String, String), String>),
    Discovered(String, Links),
    KickIds(String, u64, u64, u64),
    YouTubeLive(String, String),
    Line(String, ChatLine),
    /// A Twitch chat's recent history, oldest first (recent-messages.robotty.de).
    History(String, Vec<ChatLine>),
    YouTubeSignedIn(Result<crate::youtube_auth::YouTubeSaved, String>),
    KickSignedIn(Result<crate::kick_auth::KickSaved, String>),
    KickEmotes(String, Vec<(String, Vec<emotes::Emote>)>),
}

/// Emote and badge loads finishing in the background.
enum EmoteMsg {
    Global(emotes::Set),
    Channel(String, emotes::Set),
    GlobalBadges(HashMap<String, (String, String)>),
    ChannelBadges(String, HashMap<String, (String, String)>),
    UserTwitch(Vec<(String, emotes::Emote)>),
}

impl LiveApp {
    pub fn new(cc: &eframe::CreationContext<'_>, shot: Option<Shot>) -> Self {
        crate::style::install(&cc.egui_ctx);
        // Screenshot mode uses sample data unless LIVE_STREAMS_REAL is set (for testing the live check).
        let demo = (shot.is_some() && std::env::var("LIVE_STREAMS_REAL").is_err())
            || std::env::var("LIVE_STREAMS_DEMO").is_ok();
        let settings: Settings = if demo {
            // start from a clean slate: no remembered scroll positions or window state
            cc.egui_ctx.memory_mut(|m| m.data.clear());
            Settings::default()
        } else {
            cc.storage
                .and_then(|s| eframe::get_value(s, eframe::APP_KEY))
                .unwrap_or_default()
        };
        let (acc_tx, acc_rx) = std::sync::mpsc::channel();
        let (emotes_tx, emotes_rx) = std::sync::mpsc::channel();
        let (other_tx, other_rx) = std::sync::mpsc::channel();
        let (side_tx, side_rx) = std::sync::mpsc::channel();
        let mut app = Self {
            settings,
            streams: Vec::new(),
            list_len: 0,
            selected: None,
            worker: None,
            checking: false,
            last_check: None,
            first_done: false,
            startup_partial: false,
            error: None,
            note: None,
            chats: Vec::new(),
            active_chat: 0,
            chat_open: false,
            add_text: None,
            last_title: String::new(),
            shot,
            demo,
            chat: None,
            chat_name: None,
            mod_in: HashSet::new(),
            usercard: None,
            thread: None,
            global_search: None,
            hwnd: None,
            tray: None,
            account: None,
            account_status: "Not signed in".into(),
            account_loading: false,
            last_account_load: None,
            followed_live: None,
            acc_tx,
            acc_rx,
            acc_dialog: AccountDialog::default(),
            ctx: cc.egui_ctx.clone(),
            seen: HashMap::new(),
            removed: Vec::new(),
            show_removed: false,
            info: None,
            emotes: Store::default(),
            emotes_tx,
            emotes_rx,
            global_emotes_asked: false,
            global_badges_asked: false,
            rooms: HashMap::new(),
            readers: HashMap::new(),
            other_tx,
            other_rx,
            kick_ids: HashMap::new(),
            yt_videos: HashMap::new(),
            side_tx,
            side_rx,
            kick_account: Err("Not loaded".into()),
            links_edit: None,
            settings_ui: SettingsUi::default(),
            multi: HashSet::new(),
            open_dialog: None,
            emote_menu: false,
            emote_search: String::new(),
            pages: Vec::new(),
            active_page: 0,
            popups: Vec::new(),
            next_popup: 1,
            closed_splits: Vec::new(),
            renaming: None,
            change_channel: None,
            live_open: false,
            open_split_next: false,
            match_queue: VecDeque::new(),
            match_queued: HashSet::new(),
            matching: 0,
            applied_look: None,
            backup_checked: false,
            applied_frame: None,
            chatters: None,
            plugins: Default::default(),
            plugin_list: Vec::new(),
            matchers: Arc::new(Default::default()),
            matchers_key: String::new(),
            applied_top: None,
            emote_tab: 0,
            tab_drag: None,
            restoring: false,
            split_target: None,
        };
        app.live_open = app.settings.live_on_start;
        app.migrate_settings();
        if app.shot.is_none() {
            features::register_app_id();
        }
        app.load_plugins();
        // ask Streamlink once (in the background) which sites it can open
        let _ = crate::streamlink::supports(&app.settings.streamlink, Platform::Twitch);
        app.emotes.off = app.settings.emotes_off;
        if demo {
            app.streams = crate::demo::streams();
            if std::env::var("LIVE_STREAMS_SHOT_PLATFORMS").is_ok() {
                app.streams.truncate(3);
                app.streams.extend(crate::demo::other_platform_streams());
            }
            app.list_len = app.streams.len() + 12;
            // test-only: a theme for the screenshot (Dark, Black, Light, White)
            if let Ok(t) = std::env::var("LIVE_STREAMS_SHOT_THEME") {
                app.settings.appearance.theme = t;
            }
            if let Ok(t) = std::env::var("LIVE_STREAMS_SHOT_TABS") {
                app.settings.appearance.tab_position = t;
            }
            app.first_done = true;
            app.resort();
            // test-only: show a Settings page in the screenshot
            if let (Some(_), Ok(page)) = (&app.shot, std::env::var("LIVE_STREAMS_SHOT_SETTINGS")) {
                let p = match page.as_str() {
                    "accounts" => Page::Accounts,
                    "nicknames" => Page::Nicknames,
                    "commands" => Page::Commands,
                    "highlights" => Page::Highlights,
                    "ignores" => Page::Ignores,
                    "hotkeys" => Page::Hotkeys,
                    "notifications" => Page::Notifications,
                    "live" => Page::LiveList,
                    "platforms" => Page::Platforms,
                    "tools" => Page::ExternalTools,
                    "customization" => Page::General,
                    "filters" => Page::Filters,
                    "moderation" => Page::Moderation,
                    "plugins" => Page::Plugins,
                    "about" => Page::About,
                    _ => Page::General,
                };
                if p == Page::Platforms {
                    // sample streamers on other platforms (a Twitch tab and a Kick tab)
                    let l = |tw: &str, k: &str, y: &str, t: &str, from: &str| Links {
                        twitch: tw.into(),
                        kick: k.into(),
                        youtube: y.into(),
                        tiktok: t.into(),
                        searched: true,
                        guessed: from.contains("same name"),
                        found_from: from.into(),
                    };
                    app.settings.links.insert(
                        "northwind".into(),
                        l(
                            "northwind",
                            "northwind",
                            "@NorthwindPlays",
                            "",
                            "Twitch profile",
                        ),
                    );
                    app.settings.links.insert(
                        "kick:quillfox".into(),
                        l("", "quillfox", "", "quillfox.live", "Kick profile"),
                    );
                    app.settings.links.insert(
                        "quillfox".into(),
                        l(
                            "quillfox",
                            "quillfox",
                            "@quillfox",
                            "quillfox.live",
                            "Twitch profile",
                        ),
                    );
                    app.settings.links.insert(
                        "pixelmaple".into(),
                        l(
                            "pixelmaple",
                            "",
                            "@pixelmaple",
                            "pixelmaple",
                            "same name (please check)",
                        ),
                    );
                }
                app.open_settings(p);
            }
            // sample chats for the main window: a tab with two splits and a second tab
            let (a, b, c) = (
                app.streams[0].clone(),
                app.streams[2].clone(),
                app.streams[1].clone(),
            );
            app.open_chat(&a.login, &a.display_name);
            app.open_split_next = true;
            app.open_chat(&b.login, &b.display_name);
            app.open_chat(&c.login, &c.display_name);
            app.active_page = 0;
            app.active_chat = 0;
            // test-only: pick streams 4-6 in the live list (as Ctrl+click does); "open" also opens
            // them as tabs, "splits" as splits of one tab
            if let Ok(m) = std::env::var("LIVE_STREAMS_SHOT_MULTI") {
                let picks: Vec<String> = app
                    .shown_streams()
                    .into_iter()
                    .skip(3)
                    .take(3)
                    .map(|i| app.streams[i].login.clone())
                    .collect();
                app.multi.extend(picks);
                match m.as_str() {
                    "open" => app.open_selected(false),
                    "splits" => app.open_selected(true),
                    _ => {}
                }
                app.active_page = app.pages.len().saturating_sub(1);
            }
            // test-only: also show the emote menu / the Live Streams window inside the picture
            app.emote_menu = std::env::var("LIVE_STREAMS_SHOT_EMOTES").is_ok();
            if std::env::var("LIVE_STREAMS_SHOT_OPEN").is_ok() {
                app.open_dialog = Some((Platform::Twitch, String::new()));
            }
            if std::env::var("LIVE_STREAMS_SHOT_LIVE").is_ok() {
                cc.egui_ctx.set_embed_viewports(true);
                app.live_open = true;
            }
        } else {
            app.worker = Some(fetch::spawn_worker(cc.egui_ctx.clone()));
            app.load_kick_account();
            // reopen the tabs (with their splits) of last time
            if app.settings.restore_tabs {
                if !app.settings.tabs.is_empty() {
                    let names: HashMap<String, String> =
                        app.settings.open_tabs.iter().cloned().collect();
                    app.restoring = true;
                    for page in app.settings.tabs.clone() {
                        if page.splits.is_empty() {
                            // an empty tab stays an empty tab
                            app.pages.push(page);
                            continue;
                        }
                        for (n, key) in page.splits.iter().enumerate() {
                            app.open_split_next = n > 0;
                            let name = names
                                .get(key)
                                .cloned()
                                .unwrap_or_else(|| tab_primary(key).1);
                            app.open_chat(key, &name);
                        }
                        if let Some(last) = app.pages.last_mut() {
                            last.title = page.title.clone();
                            last.locked = page.locked;
                            last.color = page.color;
                            last.always_visible = page.always_visible;
                        }
                    }
                    app.restoring = false;
                    app.active_page = 0;
                } else if app.settings.open_pages.is_empty() {
                    for (key, name) in app.settings.open_tabs.clone() {
                        app.open_chat(&key, &name);
                    }
                } else {
                    for page in app.settings.open_pages.clone() {
                        for (n, (key, name)) in page.iter().enumerate() {
                            app.open_split_next = n > 0;
                            app.open_chat(key, name);
                        }
                    }
                    app.active_page = 0;
                }
            }
            if app.settings.account == AccountMode::Off {
                app.refresh();
            } else {
                // the first live check waits for the account so follows are included
                app.load_account();
            }
        }
        app
    }

    // --------------------------------------------------------------- account

    /// Loads the account in the background (validating it with Twitch).
    fn load_account(&mut self) {
        if self.demo || self.account_loading {
            return;
        }
        let mode = self.settings.account;
        if mode == AccountMode::Off {
            self.account = None;
            self.account_status = "Not signed in (read-only chat)".into();
            self.apply_account();
            return;
        }
        // older versions kept one sign-in; move it into the account list
        if !self.settings.device.token.is_empty() && self.settings.twitch_accounts.is_empty() {
            self.settings
                .twitch_accounts
                .push(std::mem::take(&mut self.settings.device));
        }
        let Some(saved) = self.current_saved() else {
            self.account = None;
            self.account_status = "Not signed in: add a Twitch account below".into();
            self.apply_account();
            self.refresh();
            return;
        };
        self.account_loading = true;
        self.last_account_load = Some(Instant::now());
        self.account_status = "Checking sign-in...".into();
        let tx = self.acc_tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let r = match mode {
                AccountMode::Moltorino | AccountMode::Device => {
                    account::from_device(&saved).map(|(a, s)| (a, Some(s)))
                }
                AccountMode::Off => Err("off".into()),
            };
            let _ = tx.send(AccountMsg::Loaded(r));
            ctx.request_repaint();
        });
    }

    /// The Twitch app Client ID for device login: the built-in one unless Settings overrides it.
    fn twitch_client_id(&self) -> String {
        let c = self.settings.twitch_client_id.trim();
        if c.is_empty() {
            account::DEFAULT_CLIENT_ID.to_string()
        } else {
            c.to_string()
        }
    }

    /// A tab's title: its name, or the names of its splits.
    fn page_title(&self, page: &TabPage) -> String {
        if !page.title.trim().is_empty() {
            return page.title.clone();
        }
        if page.splits.is_empty() {
            return "<empty>".to_string();
        }
        page.splits
            .iter()
            .map(|k| {
                self.chats
                    .iter()
                    .find(|t| &t.login == k)
                    .map(|t| t.display.clone())
                    .unwrap_or_else(|| k.clone())
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The current saved account (by name; else the first).
    fn current_saved(&self) -> Option<SavedDevice> {
        let list = &self.settings.twitch_accounts;
        list.iter()
            .find(|a| a.username == self.settings.current_twitch)
            .or_else(|| list.first())
            .cloned()
    }

    /// Adds or replaces an account and makes it current.
    fn add_twitch_account(&mut self, saved: SavedDevice) {
        let name = saved.username.clone();
        self.settings.twitch_accounts.retain(|a| a.username != name);
        self.settings.twitch_accounts.push(saved);
        self.settings.current_twitch = name.clone();
        self.settings.account = AccountMode::Device;
        self.acc_dialog.message = format!("Added {name}");
        self.load_account();
    }

    /// Re-reads the sign-in after Twitch refused it, at most once a minute.
    fn reload_account_soon(&mut self) {
        let recent = self
            .last_account_load
            .is_some_and(|t| t.elapsed() < Duration::from_secs(60));
        if !recent {
            self.load_account();
        }
    }

    /// Hands the account to the chat connection.
    fn apply_account(&mut self) {
        if let Some(chat) = &self.chat {
            let auth = self
                .account
                .as_ref()
                .filter(|a| a.can_chat())
                .map(|a| (a.login.clone(), a.token.clone()));
            chat.send(Cmd::Auth(auth));
        }
    }

    fn poll_account(&mut self) {
        while let Ok(msg) = self.acc_rx.try_recv() {
            match msg {
                AccountMsg::Loaded(r) => {
                    self.account_loading = false;
                    match r {
                        Ok((acc, saved)) => {
                            // keep renewed tokens
                            if let Some(s) = saved {
                                if let Some(e) = self
                                    .settings
                                    .twitch_accounts
                                    .iter_mut()
                                    .find(|a| a.username == s.username)
                                {
                                    *e = s;
                                }
                            }
                            self.settings.current_twitch = acc.login.clone();
                            let mut status = format!("Signed in as {}", acc.login);
                            if !acc.can_follow() {
                                status.push_str(" - can't read follows");
                            }
                            if !acc.can_chat() {
                                status.push_str(" - can't send messages");
                            }
                            self.account_status = status;
                            self.account = Some(acc);
                        }
                        Err(e) => {
                            self.account = None;
                            self.account_status = e;
                        }
                    }
                    self.apply_account();
                    self.load_global_badges();
                    self.load_user_emotes();
                    self.checking = false;
                    self.refresh();
                }
                AccountMsg::DeviceCode {
                    device_code,
                    user_code,
                    uri,
                    interval,
                } => {
                    // as Moltorino does: open Twitch Activate and copy the code
                    self.ctx.copy_text(user_code.clone());
                    self.acc_dialog.waiting = Some((user_code, uri.clone()));
                    self.acc_dialog.message = "Twitch Activate is open and the code is copied. Paste it there, then approve access.".into();
                    browser::open_url(&uri);
                    let client_id = self.twitch_client_id();
                    let tx = self.acc_tx.clone();
                    let ctx = self.ctx.clone();
                    std::thread::spawn(move || {
                        let started = Instant::now();
                        loop {
                            std::thread::sleep(Duration::from_secs(interval.max(1)));
                            match account::device_poll(&client_id, &device_code) {
                                account::Poll::Pending
                                    if started.elapsed() < Duration::from_secs(1800) =>
                                {
                                    continue
                                }
                                account::Poll::Pending => {
                                    let _ = tx.send(AccountMsg::DeviceDone(Err(
                                        "The code expired. Try again.".into(),
                                    )));
                                }
                                account::Poll::Done(s) => {
                                    let _ = tx.send(AccountMsg::DeviceDone(account::complete(s)));
                                }
                                account::Poll::Failed(e) => {
                                    let _ = tx.send(AccountMsg::DeviceDone(Err(e)));
                                }
                            }
                            ctx.request_repaint();
                            break;
                        }
                    });
                }
                AccountMsg::DeviceDone(r) => {
                    self.acc_dialog.waiting = None;
                    match r {
                        Ok(saved) => self.add_twitch_account(saved),
                        Err(e) => self.acc_dialog.message = e,
                    }
                }
            }
        }
    }

    fn ensure_chat(&mut self) -> &chat::Chat {
        if self.chat.is_none() {
            let c = chat::Chat::start(self.ctx.clone());
            let auth = self
                .account
                .as_ref()
                .filter(|a| a.can_chat())
                .map(|a| (a.login.clone(), a.token.clone()));
            c.send(Cmd::Auth(auth));
            self.chat = Some(c);
        }
        self.chat.as_ref().unwrap()
    }

    fn refresh(&mut self) {
        if self.demo || self.checking {
            return;
        }
        // The live list comes from the Twitch sign-in: every channel the account follows
        // (Helix, like Chatterino's / Moltorino's "followed channels"), plus channels joined
        // in a tab and added by hand. Never from a file.
        // open chats are checked too, so their split headers show live, title and viewers
        let mut extra = self.settings.extra.clone();
        extra.extend(self.chats.iter().filter_map(|t| t.twitch.clone()));
        let list = fetch::merge_lists(Vec::new(), &extra);
        // also every channel ever joined in a tab (on any platform), followed or not
        let list = known_channels(&list, &[], &self.settings.links);
        self.queue_name_matches(&list);
        self.last_check = Some(Instant::now());
        let auth = self
            .account
            .as_ref()
            .filter(|a| a.can_follow())
            .map(|a| fetch::Auth {
                client_id: a.client_id.clone(),
                token: a.token.clone(),
                user_id: a.user_id.clone(),
            });
        // how many channels the list covers: the follows (signed in) and the others
        let follows: &[String] = if auth.is_some() {
            &self.settings.follows
        } else {
            &[]
        };
        self.list_len = known_channels(&list, follows, &self.settings.links).len();
        if list.is_empty()
            && auth.is_none()
            && self.settings.links.values().all(|l| l.is_empty())
            && self.settings.matched.values().all(|l| l.is_empty())
        {
            self.error = Some(
                "Sign in with Twitch (Settings > Accounts) to see who you follow is live.".into(),
            );
            self.first_done = true;
            return;
        }
        self.error = None;
        if let Some((tx, _)) = &self.worker {
            let mut links: Vec<(String, Links)> = self
                .settings
                .links
                .iter()
                .filter(|(_, l)| !l.is_empty())
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            if self.settings.match_names {
                // channels found by name, unless a tab already set their links
                links.extend(
                    self.settings
                        .matched
                        .iter()
                        .filter(|(k, l)| !l.is_empty() && !self.settings.links.contains_key(*k))
                        .map(|(k, v)| (k.clone(), v.clone())),
                );
            }
            let open: HashSet<String> = self.chats.iter().map(|t| t.login.clone()).collect();
            if tx.send(Job::Check(list, auth, links, open)).is_ok() {
                self.checking = true;
            }
        }
    }

    fn poll(&mut self) {
        self.poll_account();
        let mut reload = false;
        let mut results = Vec::new();
        if let Some((_, rx)) = &self.worker {
            while let Ok(res) = rx.try_recv() {
                results.push(res);
            }
        }
        {
            for res in results {
                // a partial (Twitch only) result: the full one is still on its way
                self.checking = res.partial;
                self.first_done = true;
                let keep = res.error_free_or_partial();
                reload |= res.auth_failed;
                if res.followed.is_some() {
                    self.followed_live = res.followed;
                }
                if let Some(f) = &res.follows {
                    self.settings.follows = f.clone();
                    self.queue_name_matches(f);
                }
                self.error = res
                    .error
                    .clone()
                    .map(|e| format!("Live check problem: {e}"));
                if let Some(o) = &res.other {
                    for (g, uid, chan, room) in &o.kick_ids {
                        self.kick_ids.insert(g.clone(), (*uid, *chan, *room));
                    }
                    for (g, v) in &o.youtube_videos {
                        self.yt_videos.insert(g.clone(), v.clone());
                    }
                }
                if keep {
                    // a partial result never moves streams to the offline list
                    let complete = res.error.is_none() && !res.partial;
                    self.take_new_streams(res.streams, complete);
                    // the full result after a partial one still counts as the first check
                    self.startup_partial = res.partial;
                    self.start_readers();
                }
            }
        }
        let mut events = Vec::new();
        if let Some(chat) = &self.chat {
            while let Ok(e) = chat.rx.try_recv() {
                events.push(e);
            }
        }
        while let Ok(e) = self.other_rx.try_recv() {
            events.push(e);
        }
        self.poll_side();
        self.pump_name_matches();
        for e in events {
            match e {
                Event::Line(ch, line) => {
                    // Ignores (Chatterino): ignored users and blocking rules drop the message,
                    // replacing rules change its text
                    let Some(line) = self.apply_ignores(line) else {
                        continue;
                    };
                    let (mention, hit) = self.highlight_hit(&line);
                    if let Some(h) = hit {
                        self.signal_highlight(h);
                    }
                    self.log_line(&ch, &line);
                    let visible: HashSet<String> = self.visible_splits();
                    // a tab's own key (its Kick/YouTube/TikTok readers) or its Twitch channel
                    for tab in self
                        .chats
                        .iter_mut()
                        .filter(|t| t.login == ch || t.twitch.as_deref() == Some(ch.as_str()))
                    {
                        if matches!(line, ChatLine::Msg { .. }) && !visible.contains(&tab.login) {
                            tab.unread = true;
                            tab.mentioned |= mention;
                        }
                        push_line(tab, line.clone(), &self.emotes);
                    }
                }
                Event::All(line) => {
                    for tab in &mut self.chats {
                        push_line(tab, line.clone(), &self.emotes);
                    }
                }
                Event::Connected(name) => self.chat_name = name,
                Event::Room(ch, id) => self.room_found(ch, id),
                Event::ModIn(ch, yes) => {
                    if yes {
                        self.mod_in.insert(ch);
                    } else {
                        self.mod_in.remove(&ch);
                    }
                }
                Event::AuthFailed => reload = true,
            }
        }
        if reload {
            // Moltorino keeps its sign-in fresh while it runs: read it again
            self.reload_account_soon();
        }
        self.poll_emotes();
    }

    // ---------------------------------------------------------------- emotes

    fn load_global_emotes(&mut self) {
        if !self.global_emotes_asked {
            self.global_emotes_asked = true;
            let tx = self.emotes_tx.clone();
            let ctx = self.ctx.clone();
            std::thread::spawn(move || {
                let _ = tx.send(EmoteMsg::Global(emotes::load_global()));
                ctx.request_repaint();
            });
        }
        self.load_global_badges();
    }

    /// Twitch emotes the signed-in account can use, for the emote menu.
    fn load_user_emotes(&mut self) {
        let Some(acc) = self.account.clone() else {
            return;
        };
        let (tx, ctx) = (self.emotes_tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let list = emotes::load_user_emotes(&acc.client_id, &acc.token, &acc.user_id);
            let _ = tx.send(EmoteMsg::UserTwitch(list));
            ctx.request_repaint();
        });
    }

    fn load_global_badges(&mut self) {
        let Some(acc) = self.account.clone() else {
            return;
        };
        if self.global_badges_asked || self.chat.is_none() {
            return;
        }
        self.global_badges_asked = true;
        let tx = self.emotes_tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(EmoteMsg::GlobalBadges(emotes::load_badges(
                &acc.client_id,
                &acc.token,
                None,
            )));
            ctx.request_repaint();
        });
    }

    /// A channel's Twitch id arrived: load its 7TV/BTTV/FFZ emotes and badges.
    fn room_found(&mut self, ch: String, id: String) {
        if self.rooms.get(&ch) == Some(&id) {
            return;
        }
        self.rooms.insert(ch.clone(), id.clone());
        let tx = self.emotes_tx.clone();
        let ctx = self.ctx.clone();
        let acc = self.account.clone();
        std::thread::spawn(move || {
            let set = emotes::load_channel(&id);
            let _ = tx.send(EmoteMsg::Channel(ch.clone(), set));
            if let Some(acc) = acc {
                let badges = emotes::load_badges(&acc.client_id, &acc.token, Some(&id));
                let _ = tx.send(EmoteMsg::ChannelBadges(ch, badges));
            }
            ctx.request_repaint();
        });
    }

    fn poll_emotes(&mut self) {
        let mut redo: Vec<Option<String>> = Vec::new();
        while let Ok(m) = self.emotes_rx.try_recv() {
            match m {
                EmoteMsg::Global(set) => {
                    self.emotes.global = set;
                    redo.push(None);
                }
                EmoteMsg::Channel(ch, set) => {
                    let n = set.len();
                    self.emotes.channels.insert(ch.clone(), set);
                    if let Some(tab) = self.chats.iter_mut().find(|t| t.login == ch) {
                        push_line(
                            tab,
                            ChatLine::System(format!("Loaded {n} channel emotes (7TV, BTTV, FFZ)")),
                            &self.emotes,
                        );
                    }
                    redo.push(Some(ch));
                }
                EmoteMsg::GlobalBadges(b) => self.emotes.global_badges = b,
                EmoteMsg::UserTwitch(list) => self.emotes.user_twitch = list,
                EmoteMsg::ChannelBadges(ch, b) => {
                    self.emotes.channel_badges.insert(ch, b);
                }
            }
        }
        // lines that arrived before the emotes: find their emotes again
        for which in redo {
            for tab in &mut self.chats {
                if which.as_ref().is_none_or(|c| *c == tab.login) {
                    let login = tab.login.clone();
                    for line in tab.lines.iter_mut() {
                        fill_segments(line, &self.emotes, &login);
                    }
                }
            }
        }
    }

    /// Splits shown right now (the current tab and pop-out windows): they don't get unread marks.
    fn visible_splits(&self) -> HashSet<String> {
        let mut v: HashSet<String> = self
            .pages
            .get(self.active_page)
            .map(|p| p.splits.iter().cloned().collect())
            .unwrap_or_default();
        for (_, p) in &self.popups {
            v.extend(p.splits.iter().cloned());
        }
        v
    }

    /// Chatterino Commands: /trigger a b -> the command's text with {1}, {2}, {1+}, {channel}.
    fn expand_command(&self, text: &str, channel: &str) -> String {
        let mut words = text.split_whitespace();
        let Some(first) = words.next() else {
            return text.to_string();
        };
        let args: Vec<&str> = words.collect();
        let Some(cmd) = self.settings.commands.iter().find(|c| {
            let t = c.trigger.trim();
            !t.is_empty() && (t == first || format!("/{}", t.trim_start_matches('/')) == first)
        }) else {
            return text.to_string();
        };
        let mut out = cmd.text.replace("{channel}", channel);
        for i in (1..=9).rev() {
            let rest = if args.len() >= i {
                args[i - 1..].join(" ")
            } else {
                String::new()
            };
            out = out.replace(&format!("{{{i}+}}"), &rest);
            out = out.replace(&format!("{{{i}}}"), args.get(i - 1).copied().unwrap_or(""));
        }
        out
    }

    /// Emote sources switched on/off in Settings: find the emotes in every line again.
    fn apply_emote_switches(&mut self) {
        self.emotes.off = self.settings.emotes_off;
        for tab in &mut self.chats {
            let channel = tab.twitch.clone().unwrap_or_else(|| tab.login.clone());
            for line in tab.lines.iter_mut() {
                fill_segments(line, &self.emotes, &channel);
            }
        }
    }

    /// Remembers an emote picked in the emote menu (newest first).
    fn remember_emote(&mut self, e: &emotes::Emote) {
        let rec = &mut self.settings.recent_emotes;
        rec.retain(|(n, _, _)| *n != e.name);
        rec.insert(
            0,
            (
                e.name.clone(),
                e.url.clone(),
                e.provider.label().to_string(),
            ),
        );
        rec.truncate(self.settings.recent_limit);
    }

    fn set_sort(&mut self, mode: SortMode) {
        self.settings.sort = mode;
        self.resort();
    }

    fn resort(&mut self) {
        let favs = model::Favs {
            channels: Some(&self.settings.fav_channels),
            games: Some(&self.settings.fav_games),
            first: self.settings.fav_first,
        };
        model::sort_streams(&mut self.streams, self.settings.sort, &favs);
    }

    /// Takes a new live list: keeps each stream's status change time (it changes when
    /// the stream goes live or its title or game changes, as in Chatty) and moves
    /// streams that are gone into the offline list. complete = every request worked,
    /// so a missing stream really went offline.
    fn take_new_streams(&mut self, mut fresh: Vec<Stream>, complete: bool) {
        let now = model::now_unix();
        let first = (self.seen.is_empty() && self.streams.is_empty()) || self.startup_partial;
        for s in &mut fresh {
            if s.group.is_empty() {
                s.group = s.login.clone();
            }
            let changed = match self.seen.get(&s.login) {
                Some((title, game, at)) if *title == s.title && *game == s.game => *at,
                Some(_) => now,
                // already live on the first check: count from its start
                None if first => s.started,
                None => now.max(s.started),
            };
            s.status_changed = changed;
            self.seen
                .insert(s.login.clone(), (s.title.clone(), s.game.clone(), changed));
        }
        // Live Notifications (Chatterino): channels that just went live
        // Live Notifications > Options: "Suppress live notifications on startup" (the first check)
        if !first || !self.settings.notify_suppress_startup {
            let before: HashSet<&str> = self.streams.iter().map(|s| s.login.as_str()).collect();
            let selected = |s: &Stream| {
                self.settings
                    .notify_channels
                    .iter()
                    .any(|c| c.eq_ignore_ascii_case(&s.group) || c.eq_ignore_ascii_case(&s.login))
            };
            let newly: Vec<&Stream> = fresh
                .iter()
                .filter(|s| !before.contains(s.login.as_str()))
                .collect();
            // "Play sound for any channel going live" / "... for selected channels"
            if !self.demo
                && (self.settings.notify_sound_all && !newly.is_empty()
                    || self.settings.notify_sound_selected && newly.iter().any(|s| selected(s)))
            {
                features::play_sound();
            }
            let went_live: Vec<(String, String, String)> = newly
                .iter()
                .filter(|s| self.settings.notify_all || selected(s))
                .map(|s| (s.group.clone(), s.display_name.clone(), s.title.clone()))
                .collect();
            for (group, name, title) in went_live {
                // Live Notifications > Show notification: a Windows notification
                if self.settings.notify_toast && !self.demo {
                    let (tx, ctx, g, n) = (
                        self.side_tx.clone(),
                        self.ctx.clone(),
                        group.clone(),
                        name.clone(),
                    );
                    let _ = features::show_toast(&format!("{name} is live"), &title, move || {
                        let _ = tx.send(SideMsg::ToastClicked(g.clone(), n.clone()));
                        ctx.request_repaint();
                    });
                }
                self.say(format!("{name} is live: {title}"));
                self.push_to(
                    &group,
                    ChatLine::System(format!("{name} is now live: {title}")),
                );
                if self.settings.notify_flash {
                    self.ctx.send_viewport_cmd_to(
                        egui::ViewportId::ROOT,
                        egui::ViewportCommand::RequestUserAttention(
                            egui::UserAttentionType::Informational,
                        ),
                    );
                }
            }
        }
        if complete {
            let live: HashSet<&str> = fresh.iter().map(|s| s.login.as_str()).collect();
            let gone: Vec<Stream> = self
                .streams
                .iter()
                .filter(|s| !live.contains(s.login.as_str()))
                .cloned()
                .collect();
            for s in gone {
                self.seen.remove(&s.login);
                self.removed.retain(|(r, _)| r.login != s.login);
                self.removed.insert(0, (s, now));
            }
            self.removed
                .retain(|(r, _)| !live.contains(r.login.as_str()));
            self.removed.truncate(200);
        }
        self.streams = fresh;
        self.resort();
    }

    fn shown_streams(&self) -> Vec<usize> {
        let favs = model::Favs {
            channels: Some(&self.settings.fav_channels),
            games: Some(&self.settings.fav_games),
            first: false,
        };
        let mut shown: Vec<usize> = (0..self.streams.len())
            .filter(|&i| !self.settings.favs_only || favs.is_fav(&self.streams[i]))
            .collect();
        if self.settings.group_by_platform {
            // one section per platform (Twitch, Kick, YouTube, TikTok), each keeping the sort
            shown.sort_by_key(|&i| platform_order(self.streams[i].platform));
        }
        shown
    }

    fn say(&mut self, text: impl Into<String>) {
        self.note = Some((text.into(), Instant::now()));
    }

    fn open_chat(&mut self, login: &str, display: &str) {
        self.chat_open = true;
        let as_split = std::mem::take(&mut self.open_split_next);
        if let Some(i) = self.chats.iter().position(|t| t.login == login) {
            self.active_chat = i;
            self.place_split(login, as_split);
            return;
        }
        self.place_split(login, as_split);
        let (primary, name) = tab_primary(login);
        // the Twitch channel for this tab: itself, or one found for a Kick/YouTube/TikTok tab
        let twitch = match primary {
            Platform::Twitch => Some(name.clone()),
            _ => self
                .settings
                .links
                .get(login)
                .map(|l| l.twitch.clone())
                .filter(|t| !t.is_empty()),
        };
        let mut lines = VecDeque::new();
        if self.demo {
            for mut l in crate::demo::chat_lines() {
                fill_segments(&mut l, &self.emotes, login);
                lines.push_back(l);
            }
        } else {
            lines.push_back(ChatLine::System(format!(
                "Joining {} {}...",
                primary.label(),
                name
            )));
        }
        self.chats.push(ChatTab {
            login: login.into(),
            display: display.into(),
            lines,
            input: String::new(),
            twitch: twitch.clone(),
            unread: false,
            mentioned: false,
            search: None,
            history: Vec::new(),
            history_pos: 0,
            ac_sel: 0,
            ac_dismissed: None,
            tab_cycle: None,
            pending_image: None,
            reply_to: None,
        });
        self.active_chat = self.chats.len() - 1;
        if primary != Platform::Twitch {
            // a tab opened from Kick/YouTube/TikTok sends there by default
            self.settings.send_platform = primary;
        }
        if self.demo {
            // the chat-window test also loads the real global emotes to show them
            if self.shot.as_ref().is_some_and(|s| s.chats) {
                self.load_global_emotes();
            }
            return;
        }
        if let Some(tw) = twitch {
            self.join_twitch(login, tw);
        }
        self.load_global_emotes();
        self.connect_other_platforms(login);
    }

    /// Joins a tab's Twitch chat and (Settings: on) loads its recent history, like Chatterino.
    fn join_twitch(&mut self, group: &str, tw: String) {
        self.ensure_chat().send(Cmd::Join(tw.clone()));
        if !self.settings.load_history {
            return;
        }
        let (tx, ctx, g) = (self.side_tx.clone(), self.ctx.clone(), group.to_string());
        std::thread::spawn(move || {
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .timeout_global(Some(std::time::Duration::from_secs(10)))
                .build()
                .into();
            let url = format!("{}{tw}?limit=200", chat::HISTORY_URL);
            let raw: Vec<String> = agent
                .get(&url)
                .call()
                .ok()
                .and_then(|mut r| r.body_mut().read_json::<serde_json::Value>().ok())
                .and_then(|v| v.get("messages").and_then(|m| m.as_array()).cloned())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            let lines = chat::history_lines(&raw);
            if !lines.is_empty() {
                let _ = tx.send(SideMsg::History(g, lines));
                ctx.request_repaint();
            }
        });
    }

    /// Puts a chat's history in front of what arrived since, with a line between.
    fn take_history(&mut self, group: &str, history: Vec<ChatLine>) {
        let Some(tab) = self.chats.iter_mut().find(|t| t.login == group) else {
            return;
        };
        let channel = tab.twitch.clone().unwrap_or_else(|| tab.login.clone());
        let mut lines: VecDeque<ChatLine> = history
            .into_iter()
            .map(|mut l| {
                fill_segments(&mut l, &self.emotes, &channel);
                l
            })
            .collect();
        lines.push_back(ChatLine::System(
            "Older messages above (chat history from recent-messages.robotty.de)".into(),
        ));
        lines.extend(tab.lines.drain(..));
        let limit = MESSAGE_LIMIT
            .load(std::sync::atomic::Ordering::Relaxed)
            .clamp(50, 5000);
        while lines.len() > limit {
            lines.pop_front();
        }
        tab.lines = lines;
    }

    fn close_chat(&mut self, i: usize) {
        if i >= self.chats.len() {
            return;
        }
        let tab = self.chats.remove(i);
        let layout_locked = self.settings.tabs_locked;
        // a tab whose last split goes closes, unless it's locked (then it stays, empty)
        self.pages.retain_mut(|pg| {
            let had = pg.splits.contains(&tab.login);
            pg.splits.retain(|k| *k != tab.login);
            !(had && pg.splits.is_empty() && !pg.locked && !layout_locked)
        });
        for (_, pg) in &mut self.popups {
            pg.splits.retain(|k| *k != tab.login);
        }
        self.popups.retain(|(_, pg)| !pg.splits.is_empty());
        self.active_page = self.active_page.min(self.pages.len().saturating_sub(1));
        self.readers.retain(|(g, _), _| *g != tab.login);
        if let (Some(chat), Some(tw)) = (&self.chat, tab.twitch) {
            if !self
                .chats
                .iter()
                .any(|t| t.twitch.as_deref() == Some(tw.as_str()))
            {
                chat.send(Cmd::Part(tw));
            }
        }
        if self.active_chat >= self.chats.len() {
            self.active_chat = self.chats.len().saturating_sub(1);
        }
    }

    /// Opens every stream picked with Ctrl/Shift+click (in the list's order, each streamer
    /// once): each as its own tab, or all as splits of one new tab.
    fn open_selected(&mut self, as_splits: bool) {
        let mut seen = HashSet::new();
        let picked: Vec<(String, String)> = self
            .shown_streams()
            .into_iter()
            .map(|i| &self.streams[i])
            .filter(|s| self.multi.contains(&s.login))
            .map(|s| {
                (
                    if s.group.is_empty() {
                        s.login.clone()
                    } else {
                        s.group.clone()
                    },
                    s.display_name.clone(),
                )
            })
            .filter(|(g, _)| seen.insert(g.clone()))
            .collect();
        for (k, (g, n)) in picked.iter().enumerate() {
            self.open_split_next = as_splits && k > 0;
            self.open_chat(g, n);
        }
        self.open_split_next = false;
        if !picked.is_empty() {
            self.say(format!("Opened {} chats", picked.len()));
        }
        self.multi.clear();
    }

    // ------------------------------------------------------- other platforms

    /// When a tab opens: find the streamer on every platform (once: Twitch profile
    /// links, Kick profile links, then the same name), then join those chats.
    fn connect_other_platforms(&mut self, group: &str) {
        let (primary, name) = tab_primary(group);
        // a channel already found by name (live list) doesn't need looking up again
        let mut links = self
            .settings
            .links
            .get(group)
            .or_else(|| self.settings.matched.get(group))
            .cloned()
            .unwrap_or_default();
        if links.get(primary).is_empty() && primary != Platform::Twitch {
            links.set(primary, name.clone());
        }
        if !links.searched {
            let tx = self.side_tx.clone();
            let ctx = self.ctx.clone();
            let g = group.to_string();
            self.push_to(
                group,
                ChatLine::System(
                    "Looking for this streamer on Twitch, Kick, YouTube and TikTok...".into(),
                ),
            );
            std::thread::spawn(move || {
                let l = platforms::discover_from(primary, &name);
                let _ = tx.send(SideMsg::Discovered(g, l));
                ctx.request_repaint();
            });
            return;
        }
        self.lookup_links(group, &links);
    }
    /// Kick chatroom and YouTube live video for a group's links (in the background).
    fn lookup_links(&mut self, group: &str, links: &Links) {
        if !self.settings.links.contains_key(group) {
            self.settings.links.insert(group.to_string(), links.clone());
        }
        self.start_readers();
        let (tx, ctx, g, l) = (
            self.side_tx.clone(),
            self.ctx.clone(),
            group.to_string(),
            links.clone(),
        );
        std::thread::spawn(move || {
            if !l.kick.is_empty() {
                match platforms::kick_info(&l.kick) {
                    Ok(Some(k)) => {
                        let _ = tx.send(SideMsg::KickIds(
                            g.clone(),
                            k.user_id,
                            k.channel_id,
                            k.chatroom_id,
                        ));
                        let sets = emotes::load_kick_emotes(&k.slug);
                        let _ = tx.send(SideMsg::KickEmotes(g.clone(), sets));
                    }
                    Ok(None) => {
                        let _ = tx.send(SideMsg::Line(
                            g.clone(),
                            ChatLine::System(format!("Kick channel {} not found", l.kick)),
                        ));
                    }
                    Err(e) => {
                        let _ = tx.send(SideMsg::Line(
                            g.clone(),
                            ChatLine::System(format!("Kick: {e}")),
                        ));
                    }
                }
            }
            if !l.youtube.is_empty() {
                match platforms::youtube_live(&l.youtube) {
                    Ok(Some((_, video))) => {
                        let _ = tx.send(SideMsg::YouTubeLive(g.clone(), video));
                    }
                    Ok(None) => {
                        let _ = tx.send(SideMsg::Line(
                            g.clone(),
                            ChatLine::System(format!("YouTube {} is not live", l.youtube)),
                        ));
                    }
                    Err(e) => {
                        let _ = tx.send(SideMsg::Line(
                            g.clone(),
                            ChatLine::System(format!("YouTube: {e}")),
                        ));
                    }
                }
            }
            // TikTok: its reader checks whether the room is live by itself
            ctx.request_repaint();
        });
    }

    /// Name matching: queue Twitch channels not yet looked up on the other platforms.
    fn queue_name_matches(&mut self, names: &[String]) {
        if !self.settings.match_names || self.demo {
            return;
        }
        for n in names {
            if self.settings.matched.contains_key(n)
                || self.settings.links.contains_key(n)
                || !self.match_queued.insert(n.clone())
            {
                continue;
            }
            self.match_queue.push_back(n.clone());
        }
    }

    /// Runs the queued lookups a few at a time, so the sites aren't flooded.
    fn pump_name_matches(&mut self) {
        while self.settings.match_names && self.matching < MATCH_AT_ONCE {
            let Some(n) = self.match_queue.pop_front() else {
                break;
            };
            self.matching += 1;
            let (tx, ctx) = (self.side_tx.clone(), self.ctx.clone());
            std::thread::spawn(move || {
                let l = platforms::discover_from(Platform::Twitch, &n);
                let _ = tx.send(SideMsg::Matched(n, l));
                ctx.request_repaint();
            });
        }
    }

    fn poll_side(&mut self) {
        while let Ok(m) = self.side_rx.try_recv() {
            match m {
                SideMsg::ToastClicked(group, name) => match self.settings.notify_click.as_str() {
                    "Open in browser" => self.run_action(RowAction::Browser(group)),
                    "Open in Streamlink" => self.open_in_streamlink(&group),
                    "Do nothing" => {}
                    _ => {
                        self.open_chat(&group, &name);
                        self.ctx.send_viewport_cmd_to(
                            egui::ViewportId::ROOT,
                            egui::ViewportCommand::Focus,
                        );
                    }
                },
                SideMsg::Uploaded(group, r) => match r {
                    Ok((link, delete)) => {
                        if let Some(tab) = self.chats.iter_mut().find(|t| t.login == group) {
                            if !tab.input.is_empty() && !tab.input.ends_with(' ') {
                                tab.input.push(' ');
                            }
                            tab.input.push_str(&link);
                            tab.input.push(' ');
                        }
                        let note = if delete.is_empty() {
                            format!("Image uploaded: {link}")
                        } else {
                            format!("Image uploaded: {link} (delete it: {delete})")
                        };
                        self.push_to(&group, ChatLine::System(note));
                    }
                    Err(e) => self.push_to(
                        &group,
                        ChatLine::System(format!("Image upload failed: {e}")),
                    ),
                },
                SideMsg::Matched(name, l) => {
                    self.matching = self.matching.saturating_sub(1);
                    self.settings.matched.insert(name, l);
                }
                SideMsg::Discovered(g, l) => {
                    let found: Vec<String> = Platform::ALL
                        .iter()
                        .filter(|p| !l.get(**p).is_empty())
                        .map(|p| format!("{} {}", p.label(), l.get(*p)))
                        .collect();
                    let note = if found.len() <= 1 {
                        "Not found on other platforms. Right-click the stream > Platforms.. to add them.".to_string()
                    } else {
                        format!(
                            "Found via {}: {}",
                            if l.found_from.is_empty() {
                                "lookup"
                            } else {
                                &l.found_from
                            },
                            found.join(", ")
                        )
                    };
                    self.push_to(&g, ChatLine::System(note));
                    self.settings.links.insert(g.clone(), l.clone());
                    // a Kick/YouTube/TikTok tab that found its Twitch channel joins it too
                    if !l.twitch.is_empty() {
                        let tw = l.twitch.clone();
                        let mut join = false;
                        for tab in self
                            .chats
                            .iter_mut()
                            .filter(|t| t.login == g && t.twitch.is_none())
                        {
                            tab.twitch = Some(tw.clone());
                            join = true;
                        }
                        if join && !self.demo {
                            self.join_twitch(&g, tw);
                        }
                    }
                    if self.chats.iter().any(|t| t.login == g) {
                        self.lookup_links(&g, &l);
                    }
                    self.refresh();
                }
                SideMsg::KickIds(g, uid, chan, room) => {
                    self.kick_ids.insert(g, (uid, chan, room));
                    self.start_readers();
                }
                SideMsg::YouTubeLive(g, video) => {
                    self.yt_videos.insert(g, video);
                    self.start_readers();
                }
                SideMsg::Line(g, line) => self.push_to(&g, line),
                SideMsg::History(g, history) => self.take_history(&g, history),
                SideMsg::KickSignedIn(r) => {
                    self.settings_ui.kick_busy = false;
                    match r {
                        Ok(saved) => {
                            self.settings_ui.kick_message = if saved.username.is_empty() {
                                "Signed in to Kick".to_string()
                            } else {
                                format!("Signed in to Kick as {}", saved.username)
                            };
                            self.settings.kick = saved;
                            self.load_kick_account();
                        }
                        Err(e) => {
                            self.settings_ui.kick_message = format!("Kick sign-in failed: {e}")
                        }
                    }
                }
                SideMsg::KickEmotes(g, sets) => {
                    self.emotes.kick_sets.insert(g, sets);
                }
                SideMsg::YouTubeSignedIn(r) => {
                    self.settings_ui.yt_busy = false;
                    match r {
                        Ok(saved) => {
                            let name = saved.channel_name.clone();
                            self.settings.youtube = saved;
                            self.settings_ui.yt_message = format!(
                                "Signed in to YouTube{}",
                                if name.is_empty() {
                                    String::new()
                                } else {
                                    format!(" as {name}")
                                }
                            );
                        }
                        Err(e) => {
                            self.settings_ui.yt_message = format!("YouTube sign-in failed: {e}")
                        }
                    }
                }
            }
        }
    }

    fn push_to(&mut self, group: &str, line: ChatLine) {
        if let Some(tab) = self.chats.iter_mut().find(|t| t.login == group) {
            push_line(tab, line, &self.emotes);
        }
    }

    /// Starts Kick and YouTube chat readers for open tabs that have them.
    fn start_readers(&mut self) {
        if self.demo {
            return;
        }
        let groups: Vec<String> = self.chats.iter().map(|t| t.login.clone()).collect();
        for g in groups {
            if let Some((_, chan, room)) = self.kick_ids.get(&g).copied() {
                if room > 0 && !self.readers.contains_key(&(g.clone(), Platform::Kick)) {
                    let r = crate::otherchat::kick(
                        g.clone(),
                        chan,
                        room,
                        self.other_tx.clone(),
                        self.ctx.clone(),
                    );
                    self.readers
                        .insert((g.clone(), Platform::Kick), Box::new(r));
                }
            }
            if let Some(video) = self.yt_videos.get(&g).cloned() {
                if !self.readers.contains_key(&(g.clone(), Platform::YouTube)) {
                    let r = crate::otherchat::youtube(
                        g.clone(),
                        video,
                        self.other_tx.clone(),
                        self.ctx.clone(),
                    );
                    self.readers
                        .insert((g.clone(), Platform::YouTube), Box::new(r));
                }
            }
            let tiktok = self
                .settings
                .links
                .get(&g)
                .map(|l| l.tiktok.clone())
                .unwrap_or_default();
            if !tiktok.is_empty() && !self.readers.contains_key(&(g.clone(), Platform::TikTok)) {
                let r = crate::tiktokchat::start(
                    g.clone(),
                    tiktok,
                    self.other_tx.clone(),
                    self.ctx.clone(),
                );
                self.readers
                    .insert((g.clone(), Platform::TikTok), Box::new(r));
            }
        }
    }

    /// The Kick account: this app's own Kick sign-in (Moltorino's is not read).
    fn load_kick_account(&mut self) {
        let k = &self.settings.kick;
        self.kick_account = if !k.signed_in() {
            Err("Not signed in to Kick (Settings > Accounts)".into())
        } else {
            Ok((k.username.clone(), String::new()))
        };
    }

    /// Which platforms the message box can send to for this tab, with the reason when not.
    fn send_options(&self, group: &str) -> Vec<(Platform, Result<(), String>)> {
        Platform::ALL
            .iter()
            .map(|p| {
                let r = match p {
                    Platform::Twitch => {
                        let has_twitch = self.chats.iter().any(|t| t.login == group && t.twitch.is_some());
                        if !has_twitch && !self.demo {
                            Err("No Twitch channel for this tab (Platforms..)".to_string())
                        } else if self.demo || self.account.as_ref().is_some_and(|a| a.can_chat()) {
                            Ok(())
                        } else {
                            Err("Sign in to Twitch (Settings > Accounts)".to_string())
                        }
                    }
                    Platform::Kick => match (&self.kick_account, self.kick_ids.get(group)) {
                        (Err(e), _) => Err(e.clone()),
                        (Ok(_), None) => Err("No Kick channel linked (Platforms..)".into()),
                        (Ok(_), Some(_)) => Ok(()),
                    },
                    Platform::YouTube => {
                        if !self.settings.youtube.signed_in() {
                            Err("Sign in to YouTube (Settings > Accounts)".into())
                        } else if !self.yt_videos.contains_key(group) {
                            Err("This streamer isn't live on YouTube right now".into())
                        } else {
                            Ok(())
                        }
                    }
                    Platform::TikTok => Err("TikTok is read only: sending needs TikTok's anti-bot signatures, which this app does not rebuild".into()),
                };
                (*p, r)
            })
            .collect()
    }

    fn send_message(&mut self, group: String, text: String, platform: Platform) {
        let channel = tab_primary(&group).1;
        let text = self.expand_command(&text, &channel);
        if text.trim().is_empty() {
            return;
        }
        match platform {
            Platform::Twitch => {
                // a reply when message menu > Reply picked a message (used once)
                let (tw, reply) = self
                    .chats
                    .iter_mut()
                    .find(|t| t.login == group)
                    .map(|t| (t.twitch.clone(), t.reply_to.take()))
                    .unwrap_or((None, None));
                if let (Some(c), Some(tw)) = (&self.chat, tw) {
                    c.send(Cmd::Send(tw, text, reply));
                }
            }
            Platform::Kick => {
                let (Ok(_), Some((uid, _, room))) =
                    (&self.kick_account, self.kick_ids.get(&group).copied())
                else {
                    self.push_to(
                        &group,
                        ChatLine::System("Can't send to Kick: no Kick sign-in or channel".into()),
                    );
                    return;
                };
                // Kick emotes picked by name are sent the way kick.com sends them
                let text = emotes::kick_emote_codes(&text, self.emotes.kick_sets.get(&group));
                let saved = self.settings.kick.clone();
                let (tx, ctx) = (self.side_tx.clone(), self.ctx.clone());
                if saved.token.is_empty() {
                    // signed in on kick.com in the sign-in window
                    std::thread::spawn(move || {
                        if let Err(e) = crate::kick_auth::web_send(&saved.session, room, &text) {
                            let _ = tx.send(SideMsg::Line(
                                group,
                                ChatLine::System(format!("Kick: message not sent: {e}")),
                            ));
                            ctx.request_repaint();
                        }
                    });
                    return;
                }
                std::thread::spawn(move || {
                    // renew the token when it has ended, then send; Kick shows the sent
                    // message in its chat feed, so nothing is drawn here on success
                    let r = crate::kick_auth::fresh(&saved).and_then(|s| {
                        platforms::kick_send(&s.token, uid, &text)?;
                        Ok(s)
                    });
                    match r {
                        Ok(s) => {
                            if s != saved {
                                let _ = tx.send(SideMsg::KickSignedIn(Ok(s)));
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(SideMsg::Line(
                                group,
                                ChatLine::System(format!("Kick: message not sent: {e}")),
                            ));
                        }
                    }
                    ctx.request_repaint();
                });
            }
            Platform::YouTube => {
                let (Some(video), saved) = (
                    self.yt_videos.get(&group).cloned(),
                    self.settings.youtube.clone(),
                ) else {
                    self.push_to(
                        &group,
                        ChatLine::System("Can't send to YouTube: the stream isn't live".into()),
                    );
                    return;
                };
                let (tx, ctx) = (self.side_tx.clone(), self.ctx.clone());
                std::thread::spawn(move || {
                    // YouTube's chat feed shows the sent message, so nothing is drawn here on success
                    let r = if saved.refresh_token.is_empty() {
                        crate::youtube_auth::web_send(&saved.session, &video, &text)
                    } else {
                        crate::youtube_auth::send(&saved, &video, &text)
                    };
                    if let Err(e) = r {
                        let _ = tx.send(SideMsg::Line(
                            group,
                            ChatLine::System(format!("YouTube: message not sent: {e}")),
                        ));
                        ctx.request_repaint();
                    }
                });
            }
            Platform::TikTok => {
                self.push_to(
                    &group,
                    ChatLine::System("TikTok is read only in this app".into()),
                );
            }
        }
    }

    /// The "Platforms" window: a channel's Kick / YouTube / TikTok names.
    fn links_window(&mut self, ctx: &egui::Context) {
        let Some((group, mut links)) = self.links_edit.take() else {
            return;
        };
        let mut keep = true;
        let mut save = false;
        let mut find = false;
        egui::Window::new(format!("Platforms: {group}"))
            .id(egui::Id::unique("links-edit"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label("This streamer's channels on the other platforms. Opening the chat tab joins all of them.");
                if links.guessed {
                    ui.weak("Found by name: please check they are the same person.");
                }
                egui::Grid::new("links-grid").num_columns(2).show(ui, |ui| {
                    ui.label("Kick");
                    ui.text_edit_singleline(&mut links.kick);
                    ui.end_row();
                    ui.label("YouTube");
                    ui.add(egui::TextEdit::singleline(&mut links.youtube).hint_text("@handle or UC... id"));
                    ui.end_row();
                    ui.label("TikTok");
                    ui.text_edit_singleline(&mut links.tiktok);
                    ui.end_row();
                });
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        save = true;
                    }
                    if ui.button("Find automatically").on_hover_text("Kick by the same name (its profile lists YouTube and TikTok), else the same name").clicked() {
                        find = true;
                    }
                    if ui.button("Cancel").clicked() {
                        keep = false;
                    }
                });
            });
        if save {
            links.kick = links.kick.trim().trim_start_matches('@').to_lowercase();
            links.youtube = links.youtube.trim().to_string();
            links.tiktok = links.tiktok.trim().trim_start_matches('@').to_string();
            links.guessed = false;
            links.searched = true;
            self.settings.links.insert(group.clone(), links.clone());
            self.kick_ids.remove(&group);
            self.yt_videos.remove(&group);
            self.readers.retain(|(g, _), _| *g != group);
            if self.chats.iter().any(|t| t.login == group) {
                self.lookup_links(&group, &links);
            }
            self.say("Saved; the next check includes these channels");
            self.refresh();
            return;
        }
        if find {
            let (tx, ctx2, g) = (self.side_tx.clone(), ctx.clone(), group.clone());
            self.settings.links.remove(&group);
            std::thread::spawn(move || {
                let l = platforms::discover(&g);
                let _ = tx.send(SideMsg::Discovered(g, l));
                ctx2.request_repaint();
            });
            self.say("Looking for this streamer on Kick, YouTube and TikTok...");
            return;
        }
        if keep {
            self.links_edit = Some((group, links));
        }
    }

    /// Opens a stream (a list row's login, or a tab key) in Streamlink.
    fn open_in_streamlink(&mut self, key: &str) {
        // a list row (its login is kick:slug etc. for other platforms) or a tab key
        let (platform, name, group) = match self.streams.iter().find(|s| s.login == key) {
            Some(s) => {
                let (_, n) = tab_primary(&s.login);
                (s.platform, n, s.group.clone())
            }
            None => {
                let (p, n) = tab_primary(key);
                (p, n, key.to_string())
            }
        };
        self.streamlink_on(platform, &name, &group);
    }

    /// Streamlink on a given platform (Twitch, Kick, YouTube or TikTok).
    fn streamlink_on(&mut self, platform: Platform, name: &str, group: &str) {
        // YouTube: the live video when known (a channel's /live page also works)
        let video = if platform == Platform::YouTube {
            self.yt_videos.get(group).cloned()
        } else {
            None
        };
        let url = crate::streamlink::url(platform, name, video.as_deref());
        match crate::streamlink::open(&self.settings.streamlink, &url) {
            Ok(()) => self.say(format!("Opening {url} in Streamlink")),
            Err(e) => self.say(e),
        }
    }

    /// "Select a channel" (Chatterino's SelectChannelDialog): platform tabs, a channel
    /// field, "Open as split", OK / Cancel. Enter opens, Escape cancels.
    fn open_channel_window(&mut self, ctx: &egui::Context) {
        let Some((mut platform, mut name)) = self.open_dialog.take() else {
            return;
        };
        let mut keep = true;
        let mut go = false;
        let mut as_split = self.open_split_next;
        let title = if self.change_channel.is_some() {
            "Change channel"
        } else {
            "Select a channel"
        };
        egui::Window::new(title)
            .id(egui::Id::unique("open-channel"))
            .collapsible(false)
            .resizable(false)
            .title_bar(true)
            .fixed_size([360.0, 0.0])
            .anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, 80.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for p in Platform::ALL {
                        if crate::style::flat_tab(ui, p.label(), platform == p, p.color()).clicked()
                        {
                            platform = p;
                        }
                    }
                });
                ui.add_space(8.0);
                ui.label("Channel");
                let r = ui.add(
                    egui::TextEdit::singleline(&mut name)
                        .hint_text(match platform {
                            Platform::YouTube => "@handle",
                            Platform::TikTok => "@user",
                            _ => "channel name",
                        })
                        .desired_width(f32::INFINITY),
                );
                r.request_focus();
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    go = true;
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    keep = false;
                }
                ui.add_space(4.0);
                if self.change_channel.is_none() {
                    ui.checkbox(&mut as_split, "Open as a split in this tab");
                }
                ui.weak("Also joins the streamer's other platforms when found.");
                ui.add_space(6.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        keep = false;
                    }
                    if ui
                        .add_enabled(!name.trim().is_empty(), egui::Button::new("OK"))
                        .clicked()
                    {
                        go = true;
                    }
                });
            });
        self.open_split_next = as_split;
        let clean = name.trim().trim_start_matches('#').to_string();
        if go && !clean.is_empty() {
            let key = tab_key(platform, &clean);
            self.open_chat(&key, &clean);
            return;
        }
        if keep {
            self.open_dialog = Some((platform, name));
        } else {
            self.change_channel = None;
            self.open_split_next = false;
            self.split_target = None;
        }
    }
    fn title(&self) -> String {
        if self.show_removed {
            return "Offline/Left Streams".into();
        }
        // Chatty: "Live Streams ({shown or shown/total}) [Sorting: {label}]"
        let total = self.streams.len();
        let shown = self.shown_streams().len();
        let count = if shown == total {
            total.to_string()
        } else {
            format!("{shown}/{total}")
        };
        format!(
            "Live Streams ({count}) [Sorting: {}]",
            self.settings.sort.label()
        )
    }

    fn is_chat_open(&self, login: &str) -> bool {
        self.chats.iter().any(|t| t.login == login)
    }

    // ------------------------------------------------------------------ list

    fn list_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        // Settings: compiled highlights / ignores / filters, always on top
        self.refresh_matchers();
        self.poll_plugins();
        self.apply_window_options(&ctx);
        // the Chatty-style list keeps its bold Arial look (the person's picture)
        let font = FontId::new(TEXT_SIZE, crate::style::list_family());
        let now = model::now_unix();
        let mut open: Option<(String, String)> = None;
        let mut action: Option<RowAction> = None;
        let shown = self.shown_streams();

        // keyboard: up/down to select, Enter or Space to open chat, F5 to refresh
        let (up, down, enter, f5, delete) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Space),
                i.key_pressed(egui::Key::F5),
                i.key_pressed(egui::Key::Delete),
            )
        });
        let typing = ctx.egui_wants_keyboard_input();
        if f5 && !typing {
            action = Some(RowAction::Refresh);
        }
        if !self.show_removed && !shown.is_empty() && (up || down) && !typing {
            let cur = self
                .selected
                .as_ref()
                .and_then(|l| shown.iter().position(|&i| &self.streams[i].login == l));
            let next = match (cur, up) {
                (None, _) => 0,
                (Some(i), true) => i.saturating_sub(1),
                (Some(i), false) => (i + 1).min(shown.len() - 1),
            };
            self.selected = Some(self.streams[shown[next]].login.clone());
        }
        // Delete: leave the selected stream's chat (Enter joins it)
        if delete && !typing && !self.show_removed {
            if let Some(s) = self
                .selected
                .as_ref()
                .and_then(|l| self.streams.iter().find(|s| &s.login == l))
            {
                action = Some(RowAction::Leave(if s.group.is_empty() {
                    s.login.clone()
                } else {
                    s.group.clone()
                }));
            }
        }
        if enter && !typing && !self.show_removed && self.multi.len() > 1 {
            // Enter with several picked streams opens them all
            action = Some(RowAction::OpenSelected);
        } else if enter && !typing && !self.show_removed {
            if let Some(s) = self
                .selected
                .as_ref()
                .and_then(|l| self.streams.iter().find(|s| &s.login == l))
            {
                open = Some((s.group.clone(), s.display_name.clone()));
            }
        }

        let full = ui.available_rect_before_wrap();
        let bg_resp = ui.interact(full, ui.scope_id().with("list-bg"), Sense::click());
        let fav_ch = self.settings.fav_channels.clone();
        let fav_games = self.settings.fav_games.clone();
        let menu_state = MenuState {
            picked: self.multi.len(),
            sort: self.settings.sort,
            fav_first: self.settings.fav_first,
            favs_only: self.settings.favs_only,
        };

        if self.show_removed {
            self.removed_ui(ui, &font, now, &mut action);
        } else if shown.is_empty() {
            let msg = if !self.first_done || self.checking {
                format!("Checking {} channels...", self.list_len)
            } else if self.settings.favs_only && !self.streams.is_empty() {
                "None of your favourites is live. Right-click > Filter to show all.".to_string()
            } else if let Some(e) = &self.error {
                e.clone()
            } else {
                format!("None of your {} channels is live right now.", self.list_len)
            };
            ui.painter().text(
                full.center(),
                egui::Align2::CENTER_CENTER,
                msg,
                font.clone(),
                TEXT,
            );
        } else {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    // section headings only when more than one platform is live
                    let several = self.settings.group_by_platform
                        && shown.iter().any(|&i| self.streams[i].platform != self.streams[shown[0]].platform);
                    let mut section: Option<Platform> = None;
                    for &i in &shown {
                        let s = &self.streams[i];
                        if several && section != Some(s.platform) {
                            section = Some(s.platform);
                            let count = shown.iter().filter(|&&j| self.streams[j].platform == s.platform).count();
                            paint_section(ui, s.platform, count);
                        }
                        let w = ui.available_width();
                        let body = body_galley(ui, s, &font, w - LEFT - RIGHT);
                        let h = row_height(&body);
                        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
                        if ui.is_rect_visible(rect) {
                            let mut icons = String::new();
                            if fav_ch.contains(&s.login) {
                                icons.push_str("★ ");
                            }
                            if !s.game.is_empty() && fav_games.contains(&s.game) {
                                icons.push_str("◆ ");
                            }
                            if self.is_chat_open(&s.login) {
                                icons.push_str("💬 ");
                            }
                            let platform_tag = if s.platform == Platform::Twitch { String::new() } else { format!("[{}] ", s.platform.label()) };
                            let header = format!("{icons}{platform_tag}{}", model::header_text(s, now));
                            let state = RowState {
                                selected: self.selected.as_deref() == Some(s.login.as_str()) || self.multi.contains(&s.login),
                                hovered: resp.hovered(),
                                new: now - s.status_changed < model::NEW_SECONDS,
                            };
                            paint_row(ui, rect, &header, body, &font, state);
                        }
                        if resp.clicked() {
                            let (ctrl, shift) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));
                            if shift {
                                // Shift+click: every stream from the selected one to this one
                                let from = self.selected.as_ref().and_then(|l| shown.iter().position(|&j| &self.streams[j].login == l));
                                let to = shown.iter().position(|&j| j == i).unwrap_or(0);
                                let (a, b) = match from {
                                    Some(f) => (f.min(to), f.max(to)),
                                    None => (to, to),
                                };
                                if !ctrl {
                                    self.multi.clear();
                                }
                                for &j in &shown[a..=b] {
                                    self.multi.insert(self.streams[j].login.clone());
                                }
                            } else if ctrl {
                                // Ctrl+click: add or remove this stream; the one selected before counts too
                                if self.multi.is_empty() {
                                    if let Some(prev) = self.selected.clone().filter(|p| *p != s.login) {
                                        self.multi.insert(prev);
                                    }
                                }
                                if !self.multi.remove(&s.login) {
                                    self.multi.insert(s.login.clone());
                                }
                                self.selected = Some(s.login.clone());
                            } else {
                                self.multi.clear();
                                self.selected = Some(s.login.clone());
                            }
                        }
                        if resp.double_clicked() {
                            if self.multi.len() > 1 && self.multi.contains(&s.login) {
                                // double-click on one of several picked streams opens them all
                                action = Some(RowAction::OpenSelected);
                            } else {
                                open = Some((s.group.clone(), s.display_name.clone()));
                            }
                        }
                        let resp = resp.on_hover_text(format!(
                            "Double-click or Enter: open chat tab\nCtrl+click: pick several, Shift+click: pick a range; then Enter, double-click or right-click opens them all\nRight-click: Info, browser, player, Streamlink, sorting, favourites"
                        ));
                        let target = Target {
                            group: s.group.clone(),
                            login: s.login.clone(),
                            name: s.display_name.clone(),
                            game: s.game.clone(),
                            fav: fav_ch.contains(&s.login),
                            game_fav: fav_games.contains(&s.game),
                            channels: self.row_channels(s),
                            chat_open: self.is_chat_open(if s.group.is_empty() { &s.login } else { &s.group }),
                        };
                        resp.context_menu(|ui| {
                            ui.set_min_width(220.0);
                            // with several streams picked, opening them all comes first
                            if menu_state.picked > 0 {
                                picked_items(ui, &mut action, menu_state.picked);
                                ui.separator();
                            }
                            stream_menu(ui, &mut action, &target);
                            ui.separator();
                            list_menu(ui, &mut action, &menu_state, Some(&target));
                        });
                    }
                });
        }

        if !self.show_removed {
            bg_resp.context_menu(|ui| {
                ui.set_min_width(220.0);
                list_menu(ui, &mut action, &menu_state, None);
            });
        }

        if let Some((login, name)) = open {
            self.selected = Some(login.clone());
            self.open_chat(&login, &name);
        }
        if let Some(a) = action {
            self.run_action(a);
        }

        // status: small text in the bottom-right corner while checking or after a message
        let status = if let Some((t, at)) = &self.note {
            (at.elapsed() < Duration::from_secs(6)).then(|| t.clone())
        } else {
            None
        }
        .or_else(|| (self.checking && !self.streams.is_empty()).then(|| "Updating...".to_string()))
        .or_else(|| {
            (!self.streams.is_empty())
                .then(|| self.error.clone())
                .flatten()
        });
        if let Some(t) = status {
            let painter = ui.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                ui.scope_id().with("status"),
            ));
            let galley = painter.layout(
                t,
                FontId::proportional(12.0),
                TEXT,
                (full.width() - 60.0).max(100.0),
            );
            let pos = Pos2::new(
                full.right() - galley.size().x - 26.0,
                full.bottom() - galley.size().y - 10.0,
            );
            let r = Rect::from_min_size(pos, galley.size()).expand(6.0);
            painter.rect_filled(r, 4.0, Color32::from_rgba_unmultiplied(20, 20, 20, 235));
            painter.galley(pos, galley, TEXT);
        }
    }

    fn run_action(&mut self, a: RowAction) {
        match a {
            RowAction::Chat(l, n) => {
                // a Kick/YouTube/TikTok row opens its Twitch channel's tab
                let g = self
                    .streams
                    .iter()
                    .chain(self.removed.iter().map(|(s, _)| s))
                    .find(|s| s.login == l)
                    .map(|s| s.group.clone())
                    .filter(|g| !g.is_empty())
                    .unwrap_or(l);
                self.open_chat(&g, &n)
            }
            RowAction::Leave(g) => self.leave_channel(&g),
            RowAction::Info(l) => self.info = Some(l),
            RowAction::Account => self.open_settings(Page::Accounts),
            RowAction::Browser(l) => {
                let (p, n) = tab_primary(&l);
                browser::open_url(&watch_urls(p, &n).0)
            }
            RowAction::OpenUrl(u) => browser::open_url(&u),
            RowAction::Copy(l) => self.ctx.copy_text(l),
            RowAction::Sort(m) => self.set_sort(m),
            RowAction::FavFirst(on) => {
                self.settings.fav_first = on;
                self.resort();
            }
            RowAction::FavsOnly(on) => self.settings.favs_only = on,
            RowAction::Fav(l, on) => {
                if on {
                    self.settings.fav_channels.insert(l);
                } else {
                    self.settings.fav_channels.remove(&l);
                }
                self.resort();
            }
            RowAction::FavGame(g, on) => {
                if g.is_empty() {
                    return;
                }
                if on {
                    self.settings.fav_games.insert(g);
                } else {
                    self.settings.fav_games.remove(&g);
                }
                self.resort();
            }
            RowAction::Removed(on) => self.show_removed = on,
            RowAction::Streamlink(l) => self.open_in_streamlink(&l),
            RowAction::StreamlinkOn(p, n, g) => self.streamlink_on(p, &n, &g),
            RowAction::OpenSelected => self.open_selected(false),
            RowAction::OpenSelectedSplits => self.open_selected(true),
            RowAction::OpenChannel => self.open_dialog = Some((Platform::Twitch, String::new())),
            RowAction::Settings => self.open_settings(Page::Accounts),
            RowAction::Platforms(g) => {
                let l = self.settings.links.get(&g).cloned().unwrap_or_default();
                self.links_edit = Some((g, l));
            }
            RowAction::Refresh => {
                if self.demo {
                    self.say("Sample data in screenshot mode");
                } else {
                    self.refresh();
                }
            }
            RowAction::Add => self.add_text = Some(String::new()),
            // bring the main (chat) window forward
            RowAction::Chats => self
                .ctx
                .send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Focus),
        }
    }

    /// Chatty's "Offline/Left Streams" list: streams that went offline, newest first.
    fn removed_ui(
        &mut self,
        ui: &mut egui::Ui,
        font: &FontId,
        now: i64,
        action: &mut Option<RowAction>,
    ) {
        egui::Panel::bottom("removed-back")
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(10, 6)),
            )
            .show(ui, |ui| {
                if ui.button("Back to Live Streams").clicked() {
                    *action = Some(RowAction::Removed(false));
                }
            });
        egui::CentralPanel::no_frame().show(ui, |ui| {
            if self.removed.is_empty() {
                let r = ui.available_rect_before_wrap();
                ui.painter().text(
                    r.center(),
                    egui::Align2::CENTER_CENTER,
                    "No stream went offline yet.",
                    font.clone(),
                    TEXT,
                );
                return;
            }
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    for (s, at) in &self.removed {
                        let w = ui.available_width();
                        let body = body_galley(ui, s, font, w - LEFT - RIGHT);
                        let h = row_height(&body);
                        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
                        let header = format!(
                            "{} (offline {} ago, was live {})",
                            s.display_name,
                            model::format_uptime(now - at),
                            model::format_uptime(at - s.started)
                        );
                        let state = RowState {
                            selected: false,
                            hovered: resp.hovered(),
                            new: false,
                        };
                        paint_row(ui, rect, &header, body, font, state);
                        if resp.double_clicked() {
                            *action =
                                Some(RowAction::Chat(s.login.clone(), s.display_name.clone()));
                        }
                        let target = Target {
                            group: s.group.clone(),
                            login: s.login.clone(),
                            name: s.display_name.clone(),
                            game: s.game.clone(),
                            fav: self.settings.fav_channels.contains(&s.login),
                            game_fav: self.settings.fav_games.contains(&s.game),
                            channels: self.row_channels(s),
                            chat_open: self.is_chat_open(if s.group.is_empty() {
                                &s.login
                            } else {
                                &s.group
                            }),
                        };
                        resp.context_menu(|ui| {
                            ui.set_min_width(220.0);
                            stream_menu(ui, action, &target);
                            ui.separator();
                            if ui.button("Back to Live Streams").clicked() {
                                *action = Some(RowAction::Removed(false));
                                ui.close();
                            }
                        });
                    }
                });
        });
    }

    /// Chatty's "Info: channel" window.
    fn info_window(&mut self, ctx: &egui::Context) {
        let Some(login) = self.info.clone() else {
            return;
        };
        let s = self
            .streams
            .iter()
            .find(|s| s.login == login)
            .or_else(|| {
                self.removed
                    .iter()
                    .map(|(s, _)| s)
                    .find(|s| s.login == login)
            })
            .cloned();
        let live = self.streams.iter().any(|s| s.login == login);
        let mut open = true;
        let mut act: Option<RowAction> = None;
        egui::Window::new(format!("Info: {login}"))
            .id(egui::Id::unique("stream-info"))
            .open(&mut open)
            .collapsible(false)
            .default_width(420.0)
            .show(ctx, |ui| {
                let Some(s) = &s else {
                    ui.label("This stream is no longer in the list.");
                    return;
                };
                let now = model::now_unix();
                egui::Grid::new("info-grid")
                    .num_columns(2)
                    .spacing([12.0, 4.0])
                    .show(ui, |ui| {
                        ui.label("Channel");
                        ui.label(egui::RichText::new(&s.display_name).strong());
                        ui.end_row();
                        ui.label("Status");
                        ui.label(if live { "Live" } else { "Offline" });
                        ui.end_row();
                        ui.label("Title");
                        ui.add(egui::Label::new(&s.title).wrap());
                        ui.end_row();
                        ui.label("Game");
                        ui.label(if s.game.is_empty() {
                            "-"
                        } else {
                            s.game.as_str()
                        });
                        ui.end_row();
                        ui.label("Viewers");
                        ui.label(model::format_viewers(s.viewers));
                        ui.end_row();
                        ui.label("Uptime");
                        ui.label(format!(
                            "{} (since {})",
                            model::format_uptime(now - s.started),
                            chat::clock_at(s.started)
                        ));
                        ui.end_row();
                        ui.label("Last change");
                        ui.label(format!(
                            "{} ago",
                            model::format_uptime(now - s.status_changed)
                        ));
                        ui.end_row();
                    });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Open chat tab").clicked() {
                        act = Some(RowAction::Chat(s.login.clone(), s.display_name.clone()));
                    }
                    if ui.button("Open stream in browser").clicked() {
                        act = Some(RowAction::Browser(s.login.clone()));
                    }
                });
            });
        if !open {
            self.info = None;
        }
        if let Some(a) = act {
            self.run_action(a);
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(mut text) = self.add_text.take() {
            let mut keep = true;
            let mut submit = false;
            egui::Window::new("Add channel")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label("Twitch channel name(s), separated by spaces or commas:");
                    let r = ui.text_edit_singleline(&mut text);
                    r.request_focus();
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        submit = true;
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Add").clicked() {
                            submit = true;
                        }
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                    });
                });
            if submit {
                let mut added = 0;
                for part in text.split(|c: char| c == ',' || c.is_whitespace()) {
                    if let Some(n) = fetch::valid_login(part) {
                        if !self.settings.extra.contains(&n) {
                            self.settings.extra.insert(0, n);
                            added += 1;
                        }
                    }
                }
                self.say(format!("Added {added} channel(s)"));
                self.refresh();
                keep = false;
            }
            if keep {
                self.add_text = Some(text);
            }
        }
    }

    // ----------------------------------------------------------------- chats

    // ------------------------------------------------------------ screenshot

    fn screenshot_step(&mut self, ctx: &egui::Context) {
        let chats_test = self.shot.as_ref().is_some_and(|s| s.chats);
        let waiting = !self.first_done || (chats_test && self.emotes.global.len() == 0);
        let Some(shot) = &mut self.shot else { return };
        // with real data, wait (up to ~90 s) for the first live check before counting frames
        if waiting && shot.waited < 5400 {
            shot.waited += 1;
            ctx.request_repaint_after(Duration::from_millis(16));
            return;
        }
        shot.frames += 1;
        ctx.request_repaint();
        // the chat-window test waits longer so the emote pictures can download
        let at = if shot.chats { 1200 } else { 120 };
        if shot.frames == 1 {
            // Windows may cascade a new window far right and shrink it to fit; put it
            // where it fits at full size
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(Pos2::new(20.0, 20.0)));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(1570.0, 950.0)));
        }
        if shot.frames == 60 {
            if std::env::var("LIVE_STREAMS_DEBUG_SIZE").is_ok() {
                let (r, ppp) = (ctx.content_rect(), ctx.pixels_per_point());
                eprintln!(
                    "[size] content {:?} ppp {ppp} native {:?}",
                    r,
                    ctx.input(|i| i.viewport().inner_rect)
                );
            }
        }
        if shot.frames >= at && !shot.requested {
            shot.requested = true;
            let slot = shot.image.clone();
            let ctx2 = ctx.clone();
            ctx.request_screenshot(move |img| {
                *slot.lock().unwrap() = Some(img);
                ctx2.request_repaint();
            });
        }
        let img = shot.image.lock().unwrap().take();
        if let Some(img) = img {
            if let Err(e) = save_png(&shot.path, &img) {
                eprintln!("could not save screenshot: {e}");
            }
            shot.main_saved = true;
        }
        if shot.main_saved {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if shot.frames > at + 480 {
            eprintln!("screenshot never arrived");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn fill_segments(line: &mut ChatLine, store: &Store, channel: &str) {
    if let ChatLine::Msg {
        text,
        emotes: tags,
        segs,
        platform,
        own,
        ..
    } = line
    {
        if *platform == Platform::Twitch {
            // your own messages come without Twitch emote positions: find them by word
            if *own && tags.is_empty() {
                *tags = emotes::own_twitch_positions(text, &store.user_twitch);
            }
            *segs = emotes::segments(text, tags, store, channel);
        } else if segs.is_empty() {
            *segs = vec![emotes::Seg::Text(text.clone())];
        }
    }
}

/// How many messages a chat keeps (Settings > General), read by push_line.
/// Settings > Chat > Collapse repeated messages.
static COLLAPSE_REPEATS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The same user sent the same message again: count it on the previous line (×N) instead
/// of adding a line. Returns true when it was counted.
fn collapse_repeat(last: &mut ChatLine, new: &ChatLine) -> bool {
    let (
        ChatLine::Msg {
            name: n1,
            text: t1,
            platform: p1,
            segs,
            ..
        },
        ChatLine::Msg {
            name: n2,
            text: t2,
            platform: p2,
            own: false,
            ..
        },
    ) = (&mut *last, new)
    else {
        return false;
    };
    if n1 != n2 || t1 != t2 || p1 != p2 || t1.trim().is_empty() {
        return false;
    }
    if segs.is_empty() {
        segs.push(emotes::Seg::Text(t1.clone()));
    }
    // the counter is the last segment: " ×N"
    let count = match segs.last() {
        Some(emotes::Seg::Text(t)) => t.strip_prefix(" ×").and_then(|n| n.parse::<u32>().ok()),
        _ => None,
    };
    match count {
        Some(n) => {
            segs.pop();
            segs.push(emotes::Seg::Text(format!(" ×{}", n + 1)));
        }
        None => segs.push(emotes::Seg::Text(" ×2".into())),
    }
    true
}

static MESSAGE_LIMIT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(MAX_CHAT_LINES);

fn push_line(tab: &mut ChatTab, mut line: ChatLine, store: &Store) {
    let limit = MESSAGE_LIMIT
        .load(std::sync::atomic::Ordering::Relaxed)
        .clamp(50, 5000);
    let channel = tab.twitch.clone().unwrap_or_else(|| tab.login.clone());
    fill_segments(&mut line, store, &channel);
    if COLLAPSE_REPEATS.load(std::sync::atomic::Ordering::Relaxed) {
        if let Some(last) = tab.lines.back_mut() {
            if collapse_repeat(last, &line) {
                return;
            }
        }
    }
    tab.lines.push_back(line);
    while tab.lines.len() > limit {
        tab.lines.pop_front();
    }
}

enum RowAction {
    Chat(String, String),
    /// Leave this chat (its group key).
    Leave(String),
    Info(String),
    Browser(String),
    OpenUrl(String),
    /// Streamlink on one platform: (platform, channel there, group).
    StreamlinkOn(Platform, String, String),
    Copy(String),
    Sort(SortMode),
    FavFirst(bool),
    FavsOnly(bool),
    Fav(String, bool),
    FavGame(String, bool),
    Removed(bool),
    Platforms(String),
    Streamlink(String),
    OpenSelected,
    /// The picked streams as splits of one new tab.
    OpenSelectedSplits,
    OpenChannel,
    Settings,
    Refresh,
    Add,
    Chats,
    Account,
}

/// The stream a menu was opened on.
struct Target {
    login: String,
    /// The Twitch channel (chat tab) this stream belongs to.
    group: String,
    name: String,
    game: String,
    fav: bool,
    game_fav: bool,
    /// The streamer's channels: this row's platform first, then the others found for it.
    channels: Vec<(Platform, String)>,
    /// Its chat is open (offer "Leave chat").
    chat_open: bool,
}

/// A channel's page and pop-out player (Chatterino: "Open in browser", "Open player in browser").
pub fn watch_urls(p: Platform, name: &str) -> (String, Option<String>) {
    let n = name.trim();
    match p {
        Platform::Twitch => (
            format!("https://www.twitch.tv/{n}"),
            Some(format!(
                "https://player.twitch.tv/?channel={n}&parent=twitch.tv"
            )),
        ),
        Platform::Kick => (
            format!("https://kick.com/{n}"),
            Some(format!("https://player.kick.com/{n}")),
        ),
        Platform::YouTube => (format!("{}/live", platforms::youtube_url(n)), None),
        Platform::TikTok => (
            format!("https://www.tiktok.com/@{}/live", n.trim_start_matches('@')),
            None,
        ),
    }
}

/// The watch part of the channel menu, the same in the live list and in a chat split:
/// open in browser, open player in browser (per platform when there are several),
/// Streamlink, Platforms, copy name and address.
fn watch_items(
    ui: &mut egui::Ui,
    action: &mut Option<RowAction>,
    channels: &[(Platform, String)],
    key: &str,
    group: &str,
) {
    if channels.len() <= 1 {
        if let Some((p, n)) = channels.first() {
            let (page, player) = watch_urls(*p, n);
            item(ui, "Open in browser", action, || RowAction::OpenUrl(page));
            if let Some(player) = player {
                item(ui, "Open player in browser", action, || {
                    RowAction::OpenUrl(player)
                });
            }
        }
    } else {
        ui.menu_button("Open in browser", |ui| {
            for (p, n) in channels {
                let (page, _) = watch_urls(*p, n);
                item(ui, &format!("{}  {n}", p.label()), action, || {
                    RowAction::OpenUrl(page)
                });
            }
        });
        ui.menu_button("Open player in browser", |ui| {
            for (p, n) in channels {
                if let (_, Some(player)) = watch_urls(*p, n) {
                    item(ui, &format!("{}  {n}", p.label()), action, || {
                        RowAction::OpenUrl(player)
                    });
                }
            }
        });
    }
    // Streamlink has plugins for Twitch, Kick, YouTube and TikTok: one entry per platform
    // the streamer has (greyed out when the installed Streamlink lacks the plugin)
    let sl_item =
        |ui: &mut egui::Ui, action: &mut Option<RowAction>, label: &str, p: Platform, n: &str| {
            let ok = crate::streamlink::known_support(p) != Some(false);
            let r = ui.add_enabled(ok, egui::Button::new(label));
            let r = if ok {
                r
            } else {
                r.on_disabled_hover_text(format!(
                    "The installed Streamlink has no {} plugin",
                    p.label()
                ))
            };
            if r.clicked() {
                *action = Some(RowAction::StreamlinkOn(p, n.to_string(), group.to_string()));
                ui.close();
            }
        };
    if channels.len() <= 1 {
        match channels.first() {
            Some((p, n)) => sl_item(ui, action, "Open in Streamlink", *p, n),
            None => item(ui, "Open in Streamlink", action, || {
                RowAction::Streamlink(key.to_string())
            }),
        }
    } else {
        ui.menu_button("Open in Streamlink", |ui| {
            for (p, n) in channels {
                sl_item(ui, action, &format!("{}  {n}", p.label()), *p, n);
            }
        });
    }
    item(
        ui,
        "Platforms.. (Twitch, Kick, YouTube, TikTok)",
        action,
        || RowAction::Platforms(group.to_string()),
    );
    ui.separator();
    if let Some((p, n)) = channels.first() {
        item(ui, "Copy channel name", action, || {
            RowAction::Copy(n.trim_start_matches('@').to_string())
        });
        let (page, _) = watch_urls(*p, n);
        item(ui, "Copy channel URL", action, || RowAction::Copy(page));
    }
}

impl LiveApp {
    /// Every platform channel of a tab key / group: its own first, then the found ones.
    pub fn channels_of(&self, key: &str) -> Vec<(Platform, String)> {
        let (own, name) = tab_primary(key);
        let mut out = vec![(own, name)];
        if let Some(l) = self
            .settings
            .links
            .get(key)
            .or_else(|| self.settings.matched.get(key))
        {
            for p in Platform::ALL {
                let v = l.get(p).trim();
                if !v.is_empty() && !out.iter().any(|(q, _)| *q == p) {
                    out.push((p, v.to_string()));
                }
            }
        }
        out
    }

    /// A live-list row's channels: the row's platform first.
    fn row_channels(&self, s: &Stream) -> Vec<(Platform, String)> {
        let (p, n) = tab_primary(&s.login);
        let mut out = vec![(p, n)];
        let group = if s.group.is_empty() {
            &s.login
        } else {
            &s.group
        };
        for c in self.channels_of(group) {
            if !out.iter().any(|(q, _)| *q == c.0) {
                out.push(c);
            }
        }
        out
    }
}

struct MenuState {
    /// How many streams are picked with Ctrl+click.
    picked: usize,
    sort: SortMode,
    fav_first: bool,
    favs_only: bool,
}

fn item(
    ui: &mut egui::Ui,
    label: &str,
    action: &mut Option<RowAction>,
    a: impl FnOnce() -> RowAction,
) {
    if ui.button(label).clicked() {
        *action = Some(a());
        ui.close();
    }
}

/// The stream part of the menu (Chatty: "Info: name", then the stream options).
fn stream_menu(ui: &mut egui::Ui, action: &mut Option<RowAction>, t: &Target) {
    item(ui, &format!("Info: {}", t.login), action, || {
        RowAction::Info(t.login.clone())
    });
    ui.separator();
    item(ui, "Open chat tab   Enter", action, || {
        RowAction::Chat(t.login.clone(), t.name.clone())
    });
    if t.chat_open {
        item(ui, "Leave chat   Delete", action, || {
            RowAction::Leave(if t.group.is_empty() {
                t.login.clone()
            } else {
                t.group.clone()
            })
        });
    }
    watch_items(ui, action, &t.channels, &t.login, &t.group);
}

fn fav_items(ui: &mut egui::Ui, action: &mut Option<RowAction>, t: Option<&Target>) {
    let Some(t) = t else {
        ui.weak("Right-click a stream to favourite it");
        return;
    };
    if !t.fav {
        item(ui, "★ Favorite", action, || {
            RowAction::Fav(t.login.clone(), true)
        });
    } else {
        item(ui, "Unfavorite", action, || {
            RowAction::Fav(t.login.clone(), false)
        });
    }
    if !t.game.is_empty() {
        if !t.game_fav {
            item(ui, "◆ Favorite Game", action, || {
                RowAction::FavGame(t.game.clone(), true)
            });
        } else {
            item(ui, "Unfavorite Game", action, || {
                RowAction::FavGame(t.game.clone(), false)
            });
        }
    }
}

/// The list part of the menu, in Chatty's order: Sort by.., Filter, More Settings..,
/// Removed Streams.., Refresh.
/// Opening the streams picked with Ctrl/Shift+click: as tabs, or as splits of one tab.
fn picked_items(ui: &mut egui::Ui, action: &mut Option<RowAction>, n: usize) {
    item(
        ui,
        &format!("Open the {n} picked streams as tabs"),
        action,
        || RowAction::OpenSelected,
    );
    item(
        ui,
        &format!("Open the {n} picked streams as splits in one tab"),
        action,
        || RowAction::OpenSelectedSplits,
    );
}

fn list_menu(
    ui: &mut egui::Ui,
    action: &mut Option<RowAction>,
    st: &MenuState,
    t: Option<&Target>,
) {
    if st.picked > 0 {
        if t.is_none() {
            picked_items(ui, action, st.picked);
            ui.separator();
        }
    } else {
        ui.weak("Ctrl+click or Shift+click streams to pick several");
    }
    item(ui, "Open channel..", action, || RowAction::OpenChannel);
    ui.menu_button("Sort by..", |ui| {
        for m in SortMode::ALL {
            if ui
                .radio(st.sort == m, m.label())
                .on_hover_text(m.hint())
                .clicked()
            {
                *action = Some(RowAction::Sort(m));
                ui.close();
            }
        }
        ui.separator();
        let mut on = st.fav_first;
        if ui
            .checkbox(&mut on, "Favorites first")
            .on_hover_text("Favorite channels and favorite games are at the top")
            .clicked()
        {
            *action = Some(RowAction::FavFirst(on));
            ui.close();
        }
        ui.separator();
        fav_items(ui, action, t);
    });
    ui.menu_button("Filter", |ui| {
        let mut on = st.favs_only;
        if ui.checkbox(&mut on, "Show favorites only").clicked() {
            *action = Some(RowAction::FavsOnly(on));
            ui.close();
        }
        ui.separator();
        fav_items(ui, action, t);
    });
    ui.menu_button("More Settings..", |ui| {
        item(ui, "Add channel..", action, || RowAction::Add);
        item(ui, "Account (follows and sending)..", action, || {
            RowAction::Account
        });
    });
    item(ui, "Removed Streams..", action, || RowAction::Removed(true));
    ui.separator();
    item(ui, "Refresh  (F5)", action, || RowAction::Refresh);
    ui.separator();
    item(ui, "Show chat window", action, || RowAction::Chats);
    ui.weak("Alt+L shows / hides this window");
    item(ui, "Settings..", action, || RowAction::Settings);
}

/// Title and "(Game)" below the rule, wrapped to the width like Chatty's text area.
/// Lines are 15 px apart, as in the picture.
fn body_galley(ui: &egui::Ui, s: &Stream, font: &FontId, width: f32) -> Arc<egui::Galley> {
    let mut text = s.title.clone();
    if !s.game.is_empty() {
        text.push_str(&format!("\n({})", s.game));
    }
    let mut fmt = egui::TextFormat::simple(font.clone(), TEXT);
    fmt.line_height = Some(LINE_H);
    let mut job = egui::text::LayoutJob::single_section(text, fmt);
    job.wrap.max_width = width.max(40.0);
    ui.painter().layout_job(job)
}

/// One title line plus the game line gives the picture's 60 px; each wrapped
/// line adds 15 px.
fn row_height(body: &egui::Galley) -> f32 {
    let rows = body.rows.len().max(1) as f32;
    (TITLE_Y - LINE_H / 2.0) + rows * LINE_H + (ROW_H - TITLE_Y - LINE_H * 1.5)
}

fn single_line(ui: &egui::Ui, text: &str, font: &FontId, max_w: f32) -> Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::single_section(
        text.to_string(),
        egui::TextFormat::simple(font.clone(), TEXT),
    );
    job.wrap.max_width = max_w.max(20.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('…');
    ui.painter().layout_job(job)
}

/// A platform section heading in the live list: "Kick (3)" on a darker band.
fn paint_section(ui: &mut egui::Ui, platform: Platform, count: usize) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, SECTION_H), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 0.0, SECTION_BG);
    p.hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, RULE));
    p.text(
        Pos2::new(rect.left() + LEFT, rect.center().y),
        egui::Align2::LEFT_CENTER,
        format!("{} ({count})", platform.label()),
        FontId::new(TEXT_SIZE + 1.5, crate::style::list_family()),
        platform.color(),
    );
}

#[derive(Clone, Copy)]
struct RowState {
    selected: bool,
    hovered: bool,
    /// Went live or changed title/game in the last 3 minutes.
    new: bool,
}

fn paint_row(
    ui: &egui::Ui,
    rect: Rect,
    header: &str,
    body: Arc<egui::Galley>,
    font: &FontId,
    st: RowState,
) {
    let p = ui.painter();
    if st.selected {
        p.rect_filled(rect, 0.0, SELECTED);
    } else if st.hovered {
        p.rect_filled(rect, 0.0, HOVER);
    }
    let left = rect.left() + LEFT;
    let right = rect.right() - RIGHT;
    let width = right - left;

    // rule with the name centred on it; Chatty draws it differently for new and selected streams
    let rule_y = rect.top() + RULE_Y + 0.5;
    let name = single_line(ui, header, font, width - 2.0 * NAME_GAP);
    let nx = rect.center().x - name.size().x / 2.0;
    let color = if st.new {
        RULE_NEW
    } else if st.selected {
        RULE_SELECTED
    } else {
        RULE
    };
    let stroke = Stroke::new(if st.new { 2.0 } else { 1.0 }, color);
    p.hline(left..=(nx - NAME_GAP), rule_y, stroke);
    p.hline((nx + name.size().x + NAME_GAP)..=right, rule_y, stroke);
    p.galley(Pos2::new(nx, rule_y - name.size().y / 2.0), name, TEXT);

    // +1 px: with 15 px lines the text sits 1 px high compared with the picture
    p.galley(
        Pos2::new(left, rect.top() + TITLE_Y - LINE_H / 2.0 + 1.0),
        body,
        TEXT,
    );
}

fn save_png(path: &std::path::Path, img: &egui::ColorImage) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
    }
    let [w, h] = img.size;
    let mut bytes = Vec::with_capacity(w * h * 4);
    for c in &img.pixels {
        bytes.extend_from_slice(&[c.r(), c.g(), c.b(), 255]);
    }
    image::save_buffer(
        path,
        &bytes,
        w as u32,
        h as u32,
        image::ExtendedColorType::Rgba8,
    )
    .map_err(|e| e.to_string())
}

impl FetchResult {
    fn error_free_or_partial(&self) -> bool {
        // keep the old list if nothing at all came back
        self.error.is_none() || !self.streams.is_empty()
    }
}

/// Without the Windows frame the window is resized from its edges (6 px), as Chatterino's.
fn resize_edges(ctx: &egui::Context) {
    use egui::viewport::ResizeDirection as D;
    let (rect, pos, pressed, maximized) = ctx.input(|i| {
        (
            i.viewport_rect(),
            i.pointer.hover_pos(),
            i.pointer.primary_pressed(),
            i.viewport().maximized.unwrap_or(false),
        )
    });
    let Some(p) = pos else { return };
    if maximized {
        return;
    }
    let m = 6.0;
    let (l, r, t, b) = (
        p.x - rect.left() < m,
        rect.right() - p.x < m,
        p.y - rect.top() < m,
        rect.bottom() - p.y < m,
    );
    let dir = match (l, r, t, b) {
        (true, _, true, _) => Some((D::NorthWest, egui::CursorIcon::ResizeNwSe)),
        (_, true, true, _) => Some((D::NorthEast, egui::CursorIcon::ResizeNeSw)),
        (true, _, _, true) => Some((D::SouthWest, egui::CursorIcon::ResizeNeSw)),
        (_, true, _, true) => Some((D::SouthEast, egui::CursorIcon::ResizeNwSe)),
        (true, _, _, _) => Some((D::West, egui::CursorIcon::ResizeHorizontal)),
        (_, true, _, _) => Some((D::East, egui::CursorIcon::ResizeHorizontal)),
        (_, _, true, _) => Some((D::North, egui::CursorIcon::ResizeVertical)),
        (_, _, _, true) => Some((D::South, egui::CursorIcon::ResizeVertical)),
        _ => None,
    };
    if let Some((d, cursor)) = dir {
        ctx.set_cursor_icon(cursor);
        if pressed {
            ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(d));
        }
    }
}

impl eframe::App for LiveApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // Hide to tray (Windows): closing hides the window instead of quitting, as long as the
        // tray icon is there to bring it back (its menu's Quit really quits)
        if self.settings.hide_to_tray
            && self.shot.is_none()
            && !crate::tray::QUIT.load(std::sync::atomic::Ordering::Relaxed)
            && ctx.input(|i| i.viewport().close_requested())
        {
            if self.hwnd.is_none() {
                self.hwnd = crate::tray::window_id(frame);
            }
            if let Some(h) = self.hwnd {
                if self.tray.is_none() {
                    self.tray = crate::tray::create(h, ctx.clone());
                }
                if self.tray.is_some() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                    crate::tray::hide(h);
                }
            }
        }
        // Settings: compiled highlights / ignores / filters, always on top
        self.refresh_matchers();
        self.poll_plugins();
        self.apply_window_options(&ctx);
        // Chatterino's own title bar: no Windows frame, resize from the edges
        let own_bar = self.settings.appearance.own_title_bar;
        if self.applied_frame != Some(own_bar) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(!own_bar));
            self.applied_frame = Some(own_bar);
        }
        if own_bar {
            resize_edges(&ctx);
        }
        // Settings Manager: the daily automatic backup (once per start)
        if !self.backup_checked {
            self.backup_checked = true;
            if self.settings.auto_backup && !self.demo && self.shot.is_none() {
                backup::auto_backup(&self.settings);
            }
        }
        // Settings > Customization: theme, accent, font, zoom
        if self.applied_look.as_ref() != Some(&self.settings.appearance) {
            crate::style::apply(&ctx, &self.settings.appearance);
            self.applied_look = Some(self.settings.appearance.clone());
        }
        MESSAGE_LIMIT.store(
            self.settings.message_limit,
            std::sync::atomic::Ordering::Relaxed,
        );
        COLLAPSE_REPEATS.store(
            self.settings.collapse_repeats,
            std::sync::atomic::Ordering::Relaxed,
        );
        self.poll();
        if !self.demo && !self.checking {
            if let Some(t) = self.last_check {
                if t.elapsed() >= Duration::from_secs(self.settings.refresh_secs.max(30)) {
                    self.refresh();
                }
            }
        }
        // the main window is the chat (Chatterino-style); its title names the open tab
        let tab = self
            .pages
            .get(self.active_page)
            .map(|pg| self.page_title(pg))
            .unwrap_or_default();
        let title = if tab.is_empty() {
            crate::APP_ID.to_string()
        } else {
            format!("{} - {tab}", crate::APP_ID)
        };
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
        self.main_hotkeys(&ctx);
        self.main_window(ui);
        self.live_window(&ctx);
        self.dialogs(&ctx);
        self.settings_window(&ctx);
        self.open_channel_window(&ctx);
        self.links_window(&ctx);
        self.info_window(&ctx);
        self.screenshot_step(&ctx);
        // uptimes tick; check again for new streams on a timer
        ctx.request_repaint_after(Duration::from_secs(1));
    }

    fn persist_egui_memory(&self) -> bool {
        !self.demo
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if !self.demo {
            self.settings.open_tabs = self
                .chats
                .iter()
                .map(|t| (t.login.clone(), t.display.clone()))
                .collect();
            // tabs with their splits and names
            self.settings.tabs = self.pages.clone();
            self.settings.open_pages.clear();
            eframe::set_value(storage, eframe::APP_KEY, &self.settings);
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        crate::style::theme::c().window_bg.to_normalized_gamma_f32()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_messages_are_counted() {
        let m = |name: &str, text: &str| ChatLine::Msg {
            time: "12:00".into(),
            name: name.into(),
            color: None,
            text: text.into(),
            action: false,
            own: false,
            emotes: Vec::new(),
            badges: Vec::new(),
            segs: Vec::new(),
            platform: Platform::Twitch,
            msg_id: String::new(),
            user_id: String::new(),
            reply: None,
        };
        let mut last = m("amy", "hello");
        assert!(collapse_repeat(&mut last, &m("amy", "hello")));
        assert!(collapse_repeat(&mut last, &m("amy", "hello")));
        let ChatLine::Msg { segs, .. } = &last else {
            unreachable!()
        };
        assert!(matches!(segs.last(), Some(emotes::Seg::Text(t)) if t == " ×3"));
        assert!(!collapse_repeat(&mut last, &m("bob", "hello")));
        assert!(!collapse_repeat(&mut last, &m("amy", "bye")));
    }
    #[test]
    fn known_channels_include_follows_and_joined_tabs() {
        let mut links = HashMap::new();
        links.insert(
            "kick:onlykick".to_string(),
            Links {
                kick: "onlykick".into(),
                twitch: "onlykick_tv".into(),
                ..Default::default()
            },
        );
        links.insert(
            "kick:nottwitch".to_string(),
            Links {
                kick: "nottwitch".into(),
                ..Default::default()
            },
        );
        links.insert("joinedtw".to_string(), Links::default());
        links.insert("listed".to_string(), Links::default());
        let list = vec!["listed".to_string(), "extra1".to_string()];
        let follows = vec!["followed".to_string(), "listed".to_string()];
        let k = known_channels(&list, &follows, &links);
        assert_eq!(
            k,
            vec!["listed", "extra1", "followed", "joinedtw", "onlykick_tv"]
        );
    }
}
