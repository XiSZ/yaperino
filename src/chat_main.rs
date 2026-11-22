//! The main window, built like Chatterino / chatterino7 / Moltorino: a notebook of
//! tabs at the top, splits side by side in each tab, and pop-out windows.
//!
//! - Tabs: Chatterino colours (new messages #888 line, mention #ee6166, selected #555
//!   with the accent line), a red dot when live; right-click: Rename, Pop out,
//!   Move left/right, Close.
//! - Split header: one centred line in Chatterino's format
//!   "name (live) - uptime - viewers - game - title" (each part on/off), hover for a
//!   stream preview; "+" adds a split, "⋯" has Change channel, Pop out, Move,
//!   Search, Clear, Notify when live, Streamlink, Platforms, Close.
//! - Input: "Send message..." with a character counter, emote button, platform tag;
//!   Up/Down go through sent messages.
//! - Shortcuts (Chatterino's defaults, HotkeyController.cpp): Ctrl+T new split,
//!   Ctrl+Shift+T new tab, Ctrl+W close split, Ctrl+Shift+W close tab, Ctrl+R change
//!   channel, Ctrl+N pop out split, Ctrl+Shift+N pop out tab, Ctrl+G reopen split,
//!   Ctrl+F search, Ctrl+E emotes, Ctrl+P settings, Ctrl+1..8 / Ctrl+9 tabs,
//!   Ctrl+Tab / Ctrl+Shift+Tab next/previous, Alt+Left/Right focus split;
//!   Alt+L the Live Streams window (Chatty's "L").

use super::*;

/// What the header knows about a channel that is live on one platform.
struct LiveInfo {
    platform: Platform,
    viewers: u64,
    uptime: String,
    title: String,
    game: String,
    preview: Option<String>,
}

/// Where a split is shown: the main window's tab, or a pop-out window.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Host {
    Main(usize),
    Popup(u64),
}

/// Actions collected while drawing (applied afterwards).
#[derive(Default)]
struct MainActions {
    focus: Option<String>,
    close_split: Option<(Host, String)>,
    close_page: Option<usize>,
    select_page: Option<usize>,
    move_page: Option<(usize, isize)>,
    /// A tab dropped after dragging: (from, insert before this index).
    drop_page: Option<(usize, usize)>,
    lock_page: Option<usize>,
    tab_color: Option<(usize, Option<[u8; 3]>)>,
    /// Tab menu > Move to group: (tab, group; empty = none).
    set_group: Option<(usize, String)>,
    /// A group chip in the tab bar: collapse or expand that group.
    toggle_group: Option<String>,
    always_visible: Option<usize>,
    /// Local commands (/hide, /unhide, /chatters, /export): handled here, never sent.
    hide_user: Option<(String, bool)>,
    export: Option<String>,
    leave: Option<String>,
    /// Split menu > Moderation (chat modes, clear chat, shield mode).
    mod_command: Option<(String, crate::moderation::ModCommand)>,
    mod_help: Option<String>,
    open_channel: Option<String>,
    lock_layout: bool,
    move_split: Option<(Host, String, isize)>,
    rename_page: Option<usize>,
    popup_page: Option<usize>,
    popup_split: Option<String>,
    new_split: bool,
    new_tab: bool,
    change_channel: Option<String>,
    clear: Option<String>,
    toggle_search: Option<String>,
    toggle_notify: Option<String>,
    send: Option<(String, String, Platform)>,
    platform_for: Option<(String, Platform)>,
    streamlink: Option<String>,
    platforms: Option<String>,
    settings: bool,
    toggle_live: bool,
    toggle_emotes: bool,
    picked: Option<emotes::Emote>,
    /// A channel-menu item (open in browser, player, Streamlink...), as in the live list.
    row: Option<RowAction>,
    /// Account switcher (the account name at the top right): Some(None) = anonymous.
    switch_account: Option<Option<String>>,
    accounts_page: bool,
    /// Open the chatters list of this split.
    chatters: Option<String>,
    /// Split menu > Set filters: (split, filter) to switch.
    toggle_filter: Option<(String, String)>,
    /// An emote to add to / take out of the favourites.
    favourite: Option<emotes::Emote>,
    /// An emoji picked in the emote window.
    emoji: Option<String>,
    /// Tab menu: visibility (0 all, 1 only live, 2 hidden) and layout.
    set_tab_visibility: Option<u8>,
    set_tab_layout: Option<String>,
    /// A moderation action in a split's (Twitch) channel.
    moderate: Option<(String, crate::moderation::ModAction)>,
    /// A clicked "Replying to" line: show the reply thread (the split's key, thread id).
    thread: Option<(String, String)>,
    /// A clicked name: open its usercard (the split's key, the user).
    usercard: Option<(String, crate::style::UserRef)>,
    /// The sword in a split's header: moderation mode on / off for this Twitch channel.
    toggle_mod_mode: Option<String>,
    /// Image Uploader: upload this pasted image for this split.
    upload: Option<(String, Arc<egui::ColorImage>)>,
    /// Message menu > Translate: (the split's key, the message).
    translate: Option<(String, ChatLine)>,
    /// Split menu > Auto-translate this chat: on / off for this split.
    toggle_translate: Option<String>,
}

impl LiveApp {
    /// Live state of a split's channel on every platform it is live on.
    fn live_info(&self, key: &str) -> Vec<LiveInfo> {
        let now = model::now_unix();
        let twitch = self
            .chats
            .iter()
            .find(|t| t.login == key)
            .and_then(|t| t.twitch.clone());
        let bust = model::now_unix() as u64 / 300;
        self.streams
            .iter()
            .filter(|s| s.group == key || (s.platform == Platform::Twitch && twitch.as_deref() == Some(s.login.as_str())))
            .map(|s| LiveInfo {
                platform: s.platform,
                viewers: s.viewers,
                uptime: model::format_uptime(now - s.started),
                title: s.title.clone(),
                game: s.game.clone(),
                preview: match s.platform {
                    Platform::Twitch => Some(format!("https://static-cdn.jtvnw.net/previews-ttv/live_user_{}-440x248.jpg?t={bust}", s.login)),
                    Platform::YouTube => self.yt_videos.get(&s.group).map(|v| format!("https://i.ytimg.com/vi/{v}/hqdefault_live.jpg?t={bust}")),
                    _ => None,
                },
            })
            .collect()
    }

    fn chat_index(&self, key: &str) -> Option<usize> {
        self.chats.iter().position(|t| t.login == key)
    }

    /// Puts a channel into the tabs: as a new tab, or as a split in the current tab.
    pub(super) fn place_split(&mut self, key: &str, as_split: bool) {
        if self.restoring {
            // reopening last time's tabs: they're put back as they were saved
            return;
        }
        let popup = self.split_target.take();
        if let Some(target) = self.change_channel.take() {
            // Ctrl+R: the new channel takes the old split's place (in a tab or a pop-out)
            for pg in self
                .pages
                .iter_mut()
                .chain(self.popups.iter_mut().map(|(_, pg)| pg))
            {
                for k in pg.splits.iter_mut() {
                    if *k == target {
                        *k = key.to_string();
                    }
                }
            }
            let still = |pg: &TabPage| pg.splits.iter().any(|k| *k == target);
            if !self.pages.iter().any(still) && !self.popups.iter().any(|(_, pg)| still(pg)) {
                if let Some(i) = self.chat_index(&target) {
                    if target != key {
                        self.close_chat(i);
                    }
                }
            }
            let has = |pg: &TabPage| pg.splits.iter().any(|k| k == key);
            if self.pages.iter().any(has) || self.popups.iter().any(|(_, pg)| has(pg)) {
                return;
            }
        }
        // Ctrl+T / Ctrl+G in a pop-out window: the split goes into that window
        if let (Some(id), true) = (popup, as_split) {
            if let Some((_, pg)) = self.popups.iter_mut().find(|(i, _)| *i == id) {
                if !pg.splits.iter().any(|k| k == key) {
                    pg.splits.push(key.to_string());
                }
                return;
            }
        }
        if let Some(p) = self
            .pages
            .iter()
            .position(|pg| pg.splits.iter().any(|k| k == key))
        {
            self.active_page = p;
            return;
        }
        // a channel opened while an empty tab is shown goes into that tab (Chatterino)
        let into_empty = !self.restoring
            && self
                .pages
                .get(self.active_page)
                .is_some_and(|pg| pg.splits.is_empty());
        if (as_split || into_empty) && !self.pages.is_empty() {
            let p = self.active_page.min(self.pages.len() - 1);
            self.pages[p].splits.push(key.to_string());
            self.active_page = p;
        } else {
            self.pages.push(TabPage::new(key));
            self.active_page = self.pages.len() - 1;
        }
    }

    /// Removes a split from a tab or pop-out; the channel closes when nothing shows it.
    fn close_split(&mut self, host: Host, key: &str) {
        let name = self
            .chats
            .iter()
            .find(|t| t.login == key)
            .map(|t| t.display.clone())
            .unwrap_or_else(|| key.to_string());
        match host {
            Host::Main(p) => {
                let layout_locked = self.settings.tabs_locked;
                if let Some(pg) = self.pages.get_mut(p) {
                    pg.splits.retain(|k| k != key);
                    // its last split gone: the tab closes, unless it's locked (then it stays, empty)
                    if pg.splits.is_empty() && !pg.locked && !layout_locked {
                        self.pages.remove(p);
                    }
                }
            }
            Host::Popup(id) => {
                if let Some((_, pg)) = self.popups.iter_mut().find(|(i, _)| *i == id) {
                    pg.splits.retain(|k| k != key);
                }
            }
        }
        self.popups.retain(|(_, pg)| !pg.splits.is_empty());
        let shown = self
            .pages
            .iter()
            .chain(self.popups.iter().map(|(_, p)| p))
            .any(|pg| pg.splits.iter().any(|k| k == key));
        if !shown {
            if let Some(i) = self.chat_index(key) {
                self.close_chat(i);
            }
            self.closed_splits.push((key.to_string(), name));
        }
        self.active_page = self.active_page.min(self.pages.len().saturating_sub(1));
        self.show_nearest_page();
        self.focus_first_split();
    }

    /// "Only live tabs" hides offline tabs once a live check has said who's live (before
    /// that, at start, it would hide them all).
    fn live_only(&self) -> bool {
        self.settings.tab_live_only && self.live_known
    }

    /// After closing: when the tab now shown is one the tab bar hides (Only live tabs, a
    /// collapsed group), show the nearest shown one instead, the one before it first.
    /// Jumping to a hidden offline tab looked like a random chat opening.
    fn show_nearest_page(&mut self) {
        let n = self.pages.len();
        if n == 0 {
            return;
        }
        let p = self.active_page.min(n - 1);
        let shown = |q: usize| {
            let pg = &self.pages[q];
            let collapsed =
                !pg.group.is_empty() && self.settings.collapsed_groups.contains(&pg.group);
            let live = pg.splits.iter().any(|k| !self.live_info(k).is_empty());
            !collapsed && (!self.live_only() || pg.always_visible || live)
        };
        if !shown(p) {
            self.active_page = (0..p)
                .rev()
                .find(|&q| shown(q))
                .or_else(|| (p + 1..n).find(|&q| shown(q)))
                .unwrap_or(p);
        }
    }

    fn focus_first_split(&mut self) {
        if let Some(key) = self
            .pages
            .get(self.active_page)
            .and_then(|pg| pg.splits.first())
            .cloned()
        {
            if let Some(i) = self.chat_index(&key) {
                self.active_chat = i;
            }
        }
    }

    fn focus_key(&mut self, key: &str) {
        if let Some(i) = self.chat_index(key) {
            self.active_chat = i;
            if let Some(t) = self.chats.get_mut(i) {
                t.unread = false;
                t.mentioned = false;
            }
        }
    }

    fn select_page(&mut self, p: usize) {
        if p < self.pages.len() {
            self.active_page = p;
            for key in self.pages[p].splits.clone() {
                if let Some(t) = self.chats.iter_mut().find(|t| t.login == key) {
                    t.unread = false;
                    t.mentioned = false;
                }
            }
            self.focus_first_split();
        }
    }

    fn page_state(&self, p: usize, page: &TabPage) -> crate::style::TabState {
        let mut st = crate::style::TabState {
            selected: p == self.active_page,
            closable: self.settings.appearance.tab_close_buttons,
            locked: page.locked || self.settings.tabs_locked,
            hide_lock: self.settings.tabs_hide_lock,
            color: page.color,
            compact: self.settings.tabs_compact,
            ..Default::default()
        };
        for k in &page.splits {
            if let Some(t) = self.chats.iter().find(|t| &t.login == k) {
                st.unread |= t.unread;
                st.mentioned |= t.mentioned;
            }
            st.live |= self.settings.tab_live_dot && !self.live_info(k).is_empty();
        }
        st
    }

    fn focused_host(&self) -> Option<(Host, String)> {
        let key = self.chats.get(self.active_chat)?.login.clone();
        if self
            .pages
            .get(self.active_page)
            .is_some_and(|p| p.splits.contains(&key))
        {
            return Some((Host::Main(self.active_page), key));
        }
        self.popups
            .iter()
            .find(|(_, p)| p.splits.contains(&key))
            .map(|(id, _)| (Host::Popup(*id), key))
    }

    // ------------------------------------------------------------- shortcuts

    pub(super) fn main_hotkeys(&mut self, ctx: &egui::Context) {
        use egui::{Key, Modifiers};
        // Settings > Hotkeys: the person's keys, else Chatterino's defaults (hotkeys.rs);
        // nothing while a new key is being recorded there
        if self.settings_ui.recording.is_some() {
            return;
        }
        let o = self.settings.hotkeys.clone();
        let k = |id: &str| crate::hotkeys::pressed(ctx, id, &o);
        // actions with more modifiers first, so Ctrl+Shift+T isn't taken as Ctrl+T
        let new_tab = k("new_tab");
        let close_tab = k("close_tab");
        let popup_tab = k("popup_tab");
        let prev_tab = k("prev_tab");
        let alt_l = k("live_window");
        let new_split = k("new_split");
        let close_split = k("close_split");
        let join = k("join_channel");
        let leave = k("leave_channel");
        let popup_split = k("popup_split");
        let next_tab = k("next_tab");
        let settings = k("settings");
        let change = k("change_channel");
        let reopen = k("reopen_split");
        // before Ctrl+F, so Ctrl+Shift+F isn't taken as Ctrl+F
        let search_all = k("search_all");
        let search = k("search");
        let toggle_tabs = k("toggle_tabs");
        let emotes = ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::E));
        let focus_left = k("focus_left");
        let focus_right = k("focus_right");
        let mut number: Option<usize> = None;
        for (i, id) in [
            "tab_1", "tab_2", "tab_3", "tab_4", "tab_5", "tab_6", "tab_7", "tab_8", "last_tab",
        ]
        .iter()
        .enumerate()
        {
            if k(id) {
                number = Some(i);
            }
        }
        if toggle_tabs {
            self.settings.tabs_hidden = !self.settings.tabs_hidden;
        }
        if alt_l {
            self.live_open = !self.live_open;
        }
        if new_tab {
            // Chatterino: Ctrl+Shift+T opens a new, empty tab
            self.new_empty_tab();
        } else if new_split {
            self.open_split_next = true;
            self.open_dialog = Some((Platform::Twitch, String::new()));
        }
        if join {
            // Chatty / Chatterino: join a channel in a new tab
            self.open_split_next = false;
            self.open_dialog = Some((Platform::Twitch, String::new()));
        }
        if leave {
            if let Some((_, key)) = self.focused_host() {
                self.leave_channel(&key);
            }
        }
        if close_tab && !self.pages.is_empty() {
            self.close_page(self.active_page);
        } else if close_split {
            if let Some((host, key)) = self.focused_host() {
                self.close_split(host, &key);
            }
        }
        if popup_tab {
            self.popup_page(self.active_page);
        } else if popup_split {
            if let Some(key) = self.chats.get(self.active_chat).map(|t| t.login.clone()) {
                self.popup_split(&key);
            }
        }
        if !self.pages.is_empty() {
            let n = self.pages.len();
            if next_tab {
                self.select_page((self.active_page + 1) % n);
            }
            if prev_tab {
                self.select_page((self.active_page + n - 1) % n);
            }
            if let Some(i) = number {
                // Ctrl+9 is the last tab
                self.select_page(if i == 8 { n - 1 } else { i.min(n - 1) });
            }
        }
        if settings {
            self.open_settings(Page::General);
        }
        if change {
            if let Some(key) = self.chats.get(self.active_chat).map(|t| t.login.clone()) {
                self.change_channel = Some(key);
                self.open_dialog = Some((Platform::Twitch, String::new()));
            }
        }
        if reopen {
            if let Some((key, name)) = self.closed_splits.pop() {
                self.open_split_next = true;
                self.open_chat(&key, &name);
            }
        }
        if search {
            if let Some(t) = self.chats.get_mut(self.active_chat) {
                t.search = if t.search.is_some() {
                    None
                } else {
                    Some(String::new())
                };
            }
        }
        if emotes {
            self.emote_menu = !self.emote_menu;
        }
        if search_all {
            self.global_search = match self.global_search.take() {
                Some(_) => None,
                None => Some(super::search::GlobalSearch::default()),
            };
        }
        if focus_left || focus_right {
            if let Some((Host::Main(p), key)) = self.focused_host() {
                let splits = self.pages[p].splits.clone();
                if let Some(pos) = splits.iter().position(|k| *k == key) {
                    let next = if focus_left {
                        pos.saturating_sub(1)
                    } else {
                        (pos + 1).min(splits.len() - 1)
                    };
                    let k = splits[next].clone();
                    self.focus_key(&k);
                }
            }
        }
    }

    /// The split hotkeys inside a pop-out window (Chatterino popups have them too): new split
    /// (Ctrl+T, added to this window), close split (Ctrl+W), change channel, search, reopen,
    /// settings and the live list.
    fn popup_hotkeys(&mut self, ctx: &egui::Context, id: u64) {
        if self.settings_ui.recording.is_some() {
            return;
        }
        let o = self.settings.hotkeys.clone();
        let k = |a: &str| crate::hotkeys::pressed(ctx, a, &o);
        let (new_split, close_split, change, search, reopen, settings, live) = (
            k("new_split"),
            k("close_split"),
            k("change_channel"),
            k("search"),
            k("reopen_split"),
            k("settings"),
            k("live_window"),
        );
        let (join, leave) = (k("join_channel"), k("leave_channel"));
        if join {
            self.open_split_next = false;
            self.open_dialog = Some((Platform::Twitch, String::new()));
        }
        // the split shown focused in this window (else its first)
        let key = self
            .popups
            .iter()
            .find(|(i, _)| *i == id)
            .and_then(|(_, pg)| {
                let focused = self
                    .chats
                    .get(self.active_chat)
                    .map(|t| t.login.clone())
                    .filter(|k| pg.splits.contains(k));
                focused.or_else(|| pg.splits.first().cloned())
            });
        if new_split {
            self.open_split_next = true;
            self.split_target = Some(id);
            self.open_dialog = Some((Platform::Twitch, String::new()));
        }
        if reopen {
            if let Some((k, name)) = self.closed_splits.pop() {
                self.open_split_next = true;
                self.split_target = Some(id);
                self.open_chat(&k, &name);
            }
        }
        if let Some(key) = key {
            if close_split {
                self.close_split(Host::Popup(id), &key);
            } else if leave {
                self.leave_channel(&key);
            }
            if change {
                self.change_channel = Some(key.clone());
                self.open_dialog = Some((Platform::Twitch, String::new()));
            }
            if search {
                if let Some(t) = self.chats.iter_mut().find(|t| t.login == key) {
                    t.search = if t.search.is_some() {
                        None
                    } else {
                        Some(String::new())
                    };
                }
            }
        }
        if settings {
            self.open_settings(Page::General);
        }
        if live {
            self.live_open = !self.live_open;
        }
    }

    /// Leave a channel (Ctrl+L, /part, the live list's Delete): its chat closes in every tab
    /// and pop-out, and Ctrl+G brings it back.
    pub(super) fn leave_channel(&mut self, key: &str) {
        let Some(i) = self.chat_index(key) else {
            return;
        };
        let name = self.chats[i].display.clone();
        self.close_chat(i);
        self.closed_splits.push((key.to_string(), name.clone()));
        self.show_nearest_page();
        self.focus_first_split();
        self.say(format!("Left {name} (Ctrl+G joins again)"));
    }

    /// Moltorino: a clicked #channel switches to its tab, or opens it in a new one.
    fn open_channel_link(&mut self, name: &str) {
        let low = name.trim_start_matches('#').to_lowercase();
        let open = self
            .chats
            .iter()
            .find(|t| {
                t.login == low
                    || t.twitch
                        .as_deref()
                        .is_some_and(|tw| tw.eq_ignore_ascii_case(&low))
            })
            .map(|t| t.login.clone());
        if let Some(key) = open {
            if let Some(p) = self.pages.iter().position(|pg| pg.splits.contains(&key)) {
                self.active_page = p;
                self.focus_key(&key);
                return;
            }
        }
        self.open_split_next = false;
        let key = tab_key(Platform::Twitch, &low);
        self.open_chat(&key, &low);
    }

    /// Moltorino chat recording: the split's chat as text and JSON in
    /// %APPDATA%\Yaperino\Exports (split menu > Export chat, or /export).
    fn export_chat(&mut self, key: &str) {
        let Some(tab) = self.chats.iter().find(|t| t.login == key) else {
            return;
        };
        let dir = crate::paths::app_dir_or_here().join("Exports");
        let safe: String = tab
            .display
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let stamp = chrono::Local::now().format("%Y-%m-%d_%H%M%S");
        let mut text = String::new();
        let mut json = Vec::new();
        for line in &tab.lines {
            match line {
                ChatLine::Msg {
                    time,
                    name,
                    text: t,
                    platform,
                    action,
                    ..
                } => {
                    text.push_str(&format!(
                        "[{time}] [{}] {name}{}{t}\n",
                        platform.label(),
                        if *action { " " } else { ": " }
                    ));
                    json.push(serde_json::json!({ "time": time, "platform": platform.label(), "user": name, "message": t, "action": action }));
                }
                ChatLine::System(t) => {
                    text.push_str(&format!("* {t}\n"));
                    json.push(serde_json::json!({ "system": t }));
                }
                ChatLine::AutoMod(h) => {
                    text.push_str(&format!("[{}] * {}\n", h.time, h.summary()));
                    json.push(serde_json::json!({ "time": h.time, "automod": h.summary() }));
                }
            }
        }
        let base = dir.join(format!("{safe}-{stamp}"));
        let r = std::fs::create_dir_all(&dir)
            .and_then(|_| std::fs::write(base.with_extension("txt"), &text))
            .and_then(|_| {
                std::fs::write(
                    base.with_extension("json"),
                    serde_json::to_string_pretty(&json).unwrap_or_default(),
                )
            });
        let note = match r {
            Ok(()) => format!(
                "Chat exported ({} lines) to {}.txt and .json",
                tab.lines.len(),
                base.display()
            ),
            Err(e) => format!("Couldn't export the chat: {e}"),
        };
        let key = key.to_string();
        self.push_to(&key, ChatLine::System(note));
    }

    /// Chatterino's "+" / Ctrl+Shift+T: a new, empty tab, shown at once.
    fn new_empty_tab(&mut self) {
        self.pages.push(TabPage::default());
        self.active_page = self.pages.len() - 1;
    }

    /// Move a tab so it ends up before index `to` (as counted before the move); the tab
    /// that was shown stays shown. Nothing moves while it, or the tab layout, is locked.
    fn move_page_to(&mut self, from: usize, to: usize) {
        if self.settings.tabs_locked || from >= self.pages.len() || self.pages[from].locked {
            return;
        }
        self.active_page = move_item(&mut self.pages, from, to, self.active_page);
    }

    fn close_page(&mut self, p: usize) {
        // a locked tab (or a locked tab layout) can't be closed
        if self.settings.tabs_locked || self.pages.get(p).is_some_and(|pg| pg.locked) {
            return;
        }
        if self.pages.get(p).is_some_and(|pg| pg.splits.is_empty()) {
            self.pages.remove(p);
            self.active_page = self.active_page.min(self.pages.len().saturating_sub(1));
            self.show_nearest_page();
            self.focus_first_split();
            return;
        }
        if let Some(keys) = self.pages.get(p).map(|pg| pg.splits.clone()) {
            for k in keys {
                let idx = self
                    .pages
                    .iter()
                    .position(|pg| pg.splits.contains(&k))
                    .unwrap_or(p);
                self.close_split(Host::Main(idx), &k);
            }
        }
    }

    fn popup_page(&mut self, p: usize) {
        if p < self.pages.len() && !self.pages[p].locked && !self.settings.tabs_locked {
            let page = self.pages.remove(p);
            let id = self.next_popup;
            self.next_popup += 1;
            self.popups.push((id, page));
            self.active_page = self.active_page.min(self.pages.len().saturating_sub(1));
        }
    }

    /// Ctrl+N: the split moves into its own window (Chatterino "popup").
    fn popup_split(&mut self, key: &str) {
        let layout_locked = self.settings.tabs_locked;
        self.pages.retain_mut(|pg| {
            let had = pg.splits.iter().any(|k| k == key);
            pg.splits.retain(|k| k != key);
            !(had && pg.splits.is_empty() && !pg.locked && !layout_locked)
        });
        self.active_page = self.active_page.min(self.pages.len().saturating_sub(1));
        let id = self.next_popup;
        self.next_popup += 1;
        self.popups.push((id, TabPage::new(key)));
    }

    // ----------------------------------------------------------- main window

    fn chat_look(&self, search: Option<String>) -> crate::style::ChatLook {
        let me = self.account.as_ref().map(|a| a.login.clone());
        crate::style::ChatLook {
            font_size: self.settings.chat_font,
            timestamps: self.settings.timestamps,
            emote_scale: self.settings.emote_scale,
            emote_images: self.settings.show_emote_images,
            tooltips: self.settings.emote_tooltips,
            badges: self.settings.show_badges,
            alternate: self.settings.alternate_bg,
            separators: self.settings.separators,
            mention: self
                .settings
                .highlight_mentions
                .then(|| me.clone())
                .flatten()
                .map(|m| m.to_lowercase()),
            matchers: self.matchers.clone(),
            filters: Vec::new(),
            filter_skip_own: self.settings.filters_skip_own,
            favourites: self
                .settings
                .emote_favourites
                .iter()
                .map(|(n, _, _)| n.clone())
                .collect(),
            // moderation needs a Twitch sign-in with the moderation permissions
            moderation: self.account.as_ref().is_some_and(|a| a.can_moderate()),
            mod_buttons: self.settings.mod_buttons,
            timeouts: self
                .settings
                .timeout_buttons
                .iter()
                .filter_map(|t| {
                    crate::moderation::parse_duration(t)
                        .map(|s| (crate::moderation::duration_text(s), s))
                })
                .collect(),
            nicknames: self
                .settings
                .nicknames
                .iter()
                .map(|(u, n)| (u.trim().to_lowercase(), n.clone()))
                .collect(),
            search: search.map(|s| s.to_lowercase()),
            platform_badges: self.settings.platform_badges,
            source_background: self.settings.source_background,
            hidden: self
                .settings
                .hidden_users
                .iter()
                .map(|u| u.to_lowercase())
                .collect(),
            bold_names: self.settings.bold_names,
            user_colors: self.settings.user_colors,
            link_previews: self.settings.link_previews,
        }
    }

    /// Chatterino's account switcher (the account name at the top right).
    fn account_menu(&self, ui: &mut egui::Ui, act: &mut MainActions, anonymous: bool) {
        ui.set_min_width(200.0);
        ui.weak("Twitch");
        if ui.selectable_label(anonymous, "anonymous").clicked() {
            act.switch_account = Some(None);
            ui.close();
        }
        for a in &self.settings.twitch_accounts {
            let on = !anonymous && a.username == self.settings.current_twitch;
            if ui.selectable_label(on, &a.username).clicked() {
                act.switch_account = Some(Some(a.username.clone()));
                ui.close();
            }
        }
        ui.separator();
        let k = &self.settings.kick;
        ui.weak(if k.signed_in() {
            format!(
                "Kick: {}",
                if k.username.is_empty() {
                    "signed in"
                } else {
                    &k.username
                }
            )
        } else {
            "Kick: not signed in".to_string()
        });
        let y = &self.settings.youtube;
        ui.weak(if y.signed_in() {
            format!(
                "YouTube: {}",
                if y.channel_name.is_empty() {
                    "signed in"
                } else {
                    &y.channel_name
                }
            )
        } else {
            "YouTube: not signed in".to_string()
        });
        ui.separator();
        if ui.button("Manage Accounts").clicked() {
            act.accounts_page = true;
            ui.close();
        }
    }

    /// Chatterino's chatters (viewer list) popup: everyone who wrote in the split's chat,
    /// on every platform, with a search; clicking a name puts "@name " into the message box.
    fn chatters_window(&mut self, ctx: &egui::Context) {
        let Some((key, mut search)) = self.chatters.take() else {
            return;
        };
        let Some(idx) = self.chat_index(&key) else {
            return;
        };
        let mut seen: Vec<(String, Platform)> = Vec::new();
        for line in self.chats[idx].lines.iter().rev() {
            if let ChatLine::Msg {
                name,
                platform,
                own: false,
                ..
            } = line
            {
                if !seen
                    .iter()
                    .any(|(n, p)| n.eq_ignore_ascii_case(name) && p == platform)
                {
                    seen.push((name.clone(), *platform));
                }
            }
        }
        seen.sort_by_key(|(n, _)| n.to_lowercase());
        let mut open = true;
        let mut pick: Option<String> = None;
        let title = format!("Chatters - {}", self.chats[idx].display);
        egui::Window::new(title)
            .id(egui::Id::unique(("chatters", key.as_str())))
            .open(&mut open)
            .collapsible(false)
            .default_size([260.0, 420.0])
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .show(ctx, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut search)
                        .hint_text("Search")
                        .desired_width(f32::INFINITY),
                );
                let q = search.trim().to_lowercase();
                let shown: Vec<&(String, Platform)> = seen
                    .iter()
                    .filter(|(n, _)| q.is_empty() || n.to_lowercase().contains(&q))
                    .collect();
                ui.weak(format!(
                    "{} chatters in the messages seen here",
                    shown.len()
                ));
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (name, platform) in shown {
                            ui.horizontal(|ui| {
                                if *platform != Platform::Twitch {
                                    ui.label(
                                        egui::RichText::new(platform.tag())
                                            .small()
                                            .color(platform.color()),
                                    );
                                }
                                if ui
                                    .add(egui::Label::new(name.as_str()).sense(Sense::click()))
                                    .on_hover_text("Click: @mention in the message box")
                                    .clicked()
                                {
                                    pick = Some(name.clone());
                                }
                            });
                        }
                    });
            });
        if let Some(n) = pick {
            let tab = &mut self.chats[idx];
            if !tab.input.is_empty() && !tab.input.ends_with(' ') {
                tab.input.push(' ');
            }
            tab.input.push_str(&format!("@{n} "));
        }
        if open {
            self.chatters = Some((key, search));
        }
    }

    /// The tabs and the "+" (new tab). At the top or bottom they wrap onto more rows when they
    /// don't fit (Chatterino); on the left or right the column scrolls.
    fn tab_strip(&mut self, ui: &mut egui::Ui, act: &mut MainActions, vertical: bool) {
        if vertical {
            egui::ScrollArea::vertical()
                .id_salt("tab-column")
                .auto_shrink([false, false])
                .show(ui, |ui| self.tab_strip_inner(ui, act, true));
        } else {
            self.tab_strip_inner(ui, act, false);
        }
    }

    fn tab_strip_inner(&mut self, ui: &mut egui::Ui, act: &mut MainActions, vertical: bool) {
        if vertical {
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                self.tab_items(ui, act, true)
            });
        } else {
            // a row that wraps onto more rows and only takes the height they need
            ui.horizontal_wrapped(|ui| self.tab_items(ui, act, false));
        }
    }

    fn tab_items(&mut self, ui: &mut egui::Ui, act: &mut MainActions, vertical: bool) {
        {
            ui.spacing_mut().item_spacing = if vertical {
                Vec2::new(0.0, 1.0)
            } else {
                Vec2::new(1.0, 1.0)
            };
            if self.shot.is_some() && std::env::var("LIVE_STREAMS_SHOT_MOLT").is_ok() {
                // screenshot test only: coloured tabs and source backgrounds
                if let Some(first) = self.pages.first_mut() {
                    first.color = Some(TAB_COLORS[5].1);
                }
                if let Some(second) = self.pages.get_mut(1) {
                    second.color = Some(TAB_COLORS[3].1);
                }
                self.settings.source_background = true;
            }
            if self.shot.is_some() && std::env::var("LIVE_STREAMS_SHOT_LOCK").is_ok() {
                if let Some(first) = self.pages.first_mut() {
                    first.locked = true;
                }
            }
            if self.shot.is_some()
                && std::env::var("LIVE_STREAMS_SHOT_EMPTY").is_ok()
                && !self.pages.iter().any(|pg| pg.splits.is_empty())
            {
                self.new_empty_tab();
            }
            if self.shot.is_some()
                && std::env::var("LIVE_STREAMS_SHOT_MANY_TABS").is_ok()
                && self.pages.len() < 6
            {
                // screenshot test only: more tabs than fit in one row
                for name in [
                    "Northwind",
                    "PixelMaple",
                    "Quillfox",
                    "Bramblecat",
                    "Tidewalker",
                    "Lumenberry",
                    "Marigold",
                    "Orbitjay",
                    "Emberlyn",
                    "Velvetoak",
                    "Snowpetal",
                    "Fernlight",
                    "Rainhollow",
                    "Ashgrove",
                    "Driftwick",
                    "Moonfern",
                    "Sparrowind",
                    "Cobaltreed",
                    "Hazelmoor",
                    "Brightwater",
                ] {
                    self.pages.push(TabPage {
                        title: name.to_string(),
                        ..TabPage::default()
                    });
                }
                if std::env::var("LIVE_STREAMS_SHOT_GROUPS").is_ok() {
                    // two sample groups, the second one collapsed
                    for (i, pg) in self.pages.iter_mut().enumerate() {
                        pg.group = match i {
                            2..=6 => "Friends".into(),
                            7..=14 => "FPS".into(),
                            _ => String::new(),
                        };
                    }
                    self.settings.collapsed_groups = vec!["FPS".into()];
                }
            }
            let layout_locked = self.settings.tabs_locked;
            // Moltorino tab groups, in tab order (for the chips and the tab menu)
            let mut groups: Vec<String> = Vec::new();
            for pg in &self.pages {
                if !pg.group.is_empty() && !groups.contains(&pg.group) {
                    groups.push(pg.group.clone());
                }
            }
            let mut chipped: Vec<String> = Vec::new();
            // the shown tabs' places, for dropping a dragged tab between them
            let mut places: Vec<(usize, egui::Rect)> = Vec::new();
            for p in 0..self.pages.len() {
                let page = self.pages[p].clone();
                // a group's chip before its first tab: click to collapse / expand
                let collapsed =
                    !page.group.is_empty() && self.settings.collapsed_groups.contains(&page.group);
                if !page.group.is_empty() && !chipped.contains(&page.group) {
                    chipped.push(page.group.clone());
                    let n = self.pages.iter().filter(|g| g.group == page.group).count();
                    let text = format!("{} {}  {n}", if collapsed { "▸" } else { "▾" }, page.group);
                    let mut b = egui::Button::new(
                        egui::RichText::new(text).color(crate::style::theme::c().strong_text),
                    )
                    .fill(crate::style::theme::c().accent.gamma_multiply(0.3))
                    .corner_radius(4);
                    if vertical {
                        b = b.min_size(Vec2::new(ui.available_width(), 22.0));
                    }
                    if ui
                        .add(b)
                        .on_hover_text(if collapsed {
                            "Show the tabs of this group"
                        } else {
                            "Hide the tabs of this group (the shown tab stays)"
                        })
                        .clicked()
                    {
                        act.toggle_group = Some(page.group.clone());
                    }
                }
                if collapsed && p != self.active_page {
                    continue;
                }
                // General > Interface > Tab visibility: Only live tabs (the shown tab stays)
                // (Moltorino "Always visible" tabs stay)
                if self.live_only()
                    && !page.always_visible
                    && p != self.active_page
                    && !page.splits.iter().any(|k| !self.live_info(k).is_empty())
                {
                    continue;
                }
                if let Some((_, text)) = self.renaming.as_mut().filter(|(rp, _)| *rp == p) {
                    // focus the name box once; Enter or clicking elsewhere keeps the name,
                    // Esc keeps the old one (it used to grab the focus back every frame)
                    let id = ui.make_persistent_id(("rename-tab", p));
                    let r = ui.add(egui::TextEdit::singleline(text).id(id).desired_width(
                        if vertical {
                            ui.available_width()
                        } else {
                            120.0
                        },
                    ));
                    let started = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
                    if !started {
                        r.request_focus();
                        ui.data_mut(|d| d.insert_temp(id, true));
                    }
                    let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
                    let done = r.lost_focus() || (started && !r.has_focus());
                    if escape || done {
                        if !escape {
                            self.pages[p].title = text.trim().to_string();
                        }
                        self.renaming = None;
                        ui.data_mut(|d| d.remove::<bool>(id));
                    }
                    continue;
                }
                let title = self.page_title(&page);
                let min_w = if vertical { ui.available_width() } else { 0.0 };
                let locked = page.locked || layout_locked;
                let (r, close) =
                    crate::style::tab_button(ui, &title, self.page_state(p, &page), min_w);
                places.push((p, r.rect));
                let r = if page.locked && !layout_locked {
                    r.on_hover_text(
                        "Locked tab: it can't be moved or closed (right-click > Unlock tab)",
                    )
                } else if layout_locked {
                    r.on_hover_text("The tab layout is locked (right-click > Unlock tab layout)")
                } else {
                    r
                };
                if close && !locked {
                    act.close_page = Some(p);
                } else if r.clicked() {
                    act.select_page = Some(p);
                }
                if r.drag_started() && !locked {
                    self.tab_drag = Some(p);
                    act.select_page = Some(p);
                }
                if r.double_clicked() {
                    act.rename_page = Some(p);
                }
                if r.middle_clicked() && !locked {
                    act.close_page = Some(p);
                }
                let menu = TabMenu {
                    p,
                    count: self.pages.len(),
                    vertical,
                    locked,
                    page_locked: page.locked,
                    layout_locked,
                    live_only: self.settings.tab_live_only,
                    hidden: self.settings.tabs_hidden,
                    position: self.settings.appearance.tab_position.clone(),
                    color: page.color,
                    always_visible: page.always_visible,
                    group: page.group.clone(),
                    groups: groups.clone(),
                };
                r.context_menu(|ui| tab_menu(ui, act, &menu));
                if self.shot.is_some()
                    && p == 0
                    && std::env::var("LIVE_STREAMS_SHOT_MENU").as_deref() == Ok("tab")
                {
                    // screenshot test only: the tab menu shown open
                    egui::Area::new(ui.scope_id().with("shot-tab-menu"))
                        .fixed_pos(r.rect.left_bottom() + Vec2::new(4.0, 2.0))
                        .order(egui::Order::Foreground)
                        .show(ui.ctx(), |ui| {
                            egui::Frame::menu(ui.style())
                                .show(ui, |ui| tab_menu(ui, &mut MainActions::default(), &menu))
                        });
                }
            }
            // a tab being dragged: show where it would go, move it there on release
            if let Some(from) = self.tab_drag {
                let pointer = ui.ctx().pointer_interact_pos();
                let released = ui.input(|i| !i.pointer.primary_down());
                if layout_locked || from >= self.pages.len() || pointer.is_none() {
                    self.tab_drag = None;
                } else if let Some(pos) = pointer {
                    let before = drop_before(&places, pos, vertical);
                    let to = before.map(|i| places[i].0).unwrap_or(self.pages.len());
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    // the insertion line
                    let edge = match before {
                        Some(i) => places[i].1,
                        None => places
                            .last()
                            .map(|(_, r)| *r)
                            .unwrap_or(egui::Rect::NOTHING),
                    };
                    if edge.is_positive() && to != from && to != from + 1 {
                        let col = crate::style::theme::c().accent;
                        let p = ui.painter();
                        if vertical {
                            let y = if before.is_some() {
                                edge.top() - 1.0
                            } else {
                                edge.bottom() + 1.0
                            };
                            p.line_segment(
                                [egui::pos2(edge.left(), y), egui::pos2(edge.right(), y)],
                                egui::Stroke::new(2.0, col),
                            );
                        } else {
                            let x = if before.is_some() {
                                edge.left() - 1.0
                            } else {
                                edge.right() + 1.0
                            };
                            p.line_segment(
                                [egui::pos2(x, edge.top()), egui::pos2(x, edge.bottom())],
                                egui::Stroke::new(2.0, col),
                            );
                        }
                    }
                    if released {
                        act.drop_page = Some((from, to));
                        self.tab_drag = None;
                    }
                }
            }
            ui.add_space(4.0);
            if crate::style::bar_icon_sized(ui, crate::style::BarIcon::Plus, Vec2::new(26.0, 24.0))
                .on_hover_text("New tab (Ctrl+Shift+T)")
                .clicked()
            {
                act.new_tab = true;
            }
        }
    }

    pub(super) fn main_window(&mut self, ui: &mut egui::Ui) {
        let mut act = MainActions::default();
        let me = self.account.as_ref().map(|a| a.login.clone());

        // ---- Chatterino's title bar: (drag to move, double-click to maximize) the live
        // channels, the account name (account switcher), settings, then the window buttons
        let own_bar = self.settings.appearance.own_title_bar;
        egui::Panel::top("title-bar")
            .exact_size(30.0)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(crate::style::theme::c().window_bg))
            .show(ui, |ui| {
                let bar = ui.max_rect();
                let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                if own_bar {
                    let drag = ui.interact(
                        bar,
                        ui.scope_id().with("title-drag"),
                        Sense::click_and_drag(),
                    );
                    if drag.drag_started_by(egui::PointerButton::Primary) {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                    if drag.double_clicked() {
                        ui.ctx()
                            .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                    }
                }
                ui.horizontal_centered(|ui| {
                    ui.add_space(8.0);
                    if own_bar {
                        let tex = self.logo_texture(ui.ctx());
                        ui.add(egui::Image::new(&tex).fit_to_exact_size(Vec2::splat(16.0)));
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(crate::APP_ID)
                                .size(12.0)
                                .color(crate::style::theme::c().tab_text),
                        );
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);
                        if own_bar {
                            if crate::style::window_button(ui, crate::style::BarIcon::Close)
                                .on_hover_text("Close")
                                .clicked()
                            {
                                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                            let max_icon = if maximized {
                                crate::style::BarIcon::Restore
                            } else {
                                crate::style::BarIcon::Maximize
                            };
                            if crate::style::window_button(ui, max_icon)
                                .on_hover_text(if maximized { "Restore" } else { "Maximize" })
                                .clicked()
                            {
                                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(
                                    !maximized,
                                ));
                            }
                            if crate::style::window_button(ui, crate::style::BarIcon::Minimize)
                                .on_hover_text("Minimize")
                                .clicked()
                            {
                                ui.ctx()
                                    .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                            }
                            ui.add_space(6.0);
                        }
                        if crate::style::bar_icon(ui, crate::style::BarIcon::Gear)
                            .on_hover_text("Settings (Ctrl+P)")
                            .clicked()
                        {
                            act.settings = true;
                        }
                        if self.settings.tabs_hidden
                            && ui
                                .add(
                                    egui::Button::new(egui::RichText::new("Show tabs").size(12.0))
                                        .frame_when_inactive(false),
                                )
                                .on_hover_text("The tabs are hidden (Ctrl+U)")
                                .clicked()
                        {
                            act.set_tab_visibility =
                                Some(if self.settings.tab_live_only { 1 } else { 0 });
                        }
                        ui.add_space(2.0);
                        let anonymous = self.settings.account == AccountMode::Off || me.is_none();
                        let who = me
                            .clone()
                            .filter(|_| !anonymous)
                            .unwrap_or_else(|| "anonymous".into());
                        let button = egui::Button::new(
                            egui::RichText::new(who)
                                .size(12.5)
                                .strong()
                                .color(crate::style::theme::c().strong_text),
                        )
                        .frame_when_inactive(false);
                        let r = egui::containers::menu::MenuButton::from_button(button)
                            .ui(ui, |ui| self.account_menu(ui, &mut act, anonymous))
                            .0;
                        if self.shot.is_some()
                            && std::env::var("LIVE_STREAMS_SHOT_MENU").as_deref() == Ok("account")
                        {
                            // screenshot test only: the account switcher shown open
                            egui::Area::new(egui::Id::unique("shot-account-menu"))
                                .fixed_pos(egui::Pos2::new(
                                    r.rect.right() - 210.0,
                                    r.rect.bottom() + 2.0,
                                ))
                                .order(egui::Order::Foreground)
                                .show(ui.ctx(), |ui| {
                                    egui::Frame::menu(ui.style()).show(ui, |ui| {
                                        self.account_menu(
                                            ui,
                                            &mut MainActions::default(),
                                            anonymous,
                                        )
                                    })
                                });
                        }
                    });
                });
            });
        // ---- the notebook: tabs (and "+" for a new tab), like Chatterino's; Customization >
        // Tabs puts them at the top, left, right or bottom (Chatterino's tab direction)
        let tab_frame = egui::Frame::new()
            .fill(crate::style::theme::c().window_bg)
            .inner_margin(egui::Margin {
                left: 2,
                right: 2,
                top: 2,
                bottom: 2,
            });
        let layout = if self.settings.tabs_hidden {
            "Hidden".to_string()
        } else {
            self.settings.appearance.tab_position.clone()
        };
        match layout.as_str() {
            // Ctrl+U / tab menu > Tab visibility > Hide all tabs
            "Hidden" => {}
            "Left" => {
                egui::Panel::left("main-tabs-left")
                    .exact_size(170.0)
                    .resizable(false)
                    .frame(tab_frame)
                    .show(ui, |ui| self.tab_strip(ui, &mut act, true));
            }
            "Right" => {
                egui::Panel::right("main-tabs-right")
                    .exact_size(170.0)
                    .resizable(false)
                    .frame(tab_frame)
                    .show(ui, |ui| self.tab_strip(ui, &mut act, true));
            }
            "Bottom" => {
                egui::Panel::bottom("main-tabs-bottom")
                    .frame(tab_frame)
                    .show(ui, |ui| self.tab_strip(ui, &mut act, false));
            }
            _ => {
                egui::Panel::top("main-tabs")
                    .frame(tab_frame)
                    .show(ui, |ui| self.tab_strip(ui, &mut act, false));
            }
        }
        // ---- the splits of the current tab
        egui::CentralPanel::no_frame()
            .frame(egui::Frame::new().fill(crate::style::theme::c().window_bg))
            .show(ui, |ui| {
                let Some(page) = self.pages.get(self.active_page).cloned() else {
                    empty_hint(ui);
                    return;
                };
                if page.splits.is_empty() {
                    // an empty tab: add its first split
                    if empty_tab_hint(ui) {
                        act.new_split = true;
                    }
                    return;
                }
                self.splits_row(ui, &page.splits, Host::Main(self.active_page), &mut act);
            });

        self.emote_menu_window(ui.ctx(), &mut act);
        self.chatters_window(ui.ctx());
        self.thread_window(ui.ctx(), &mut act);
        self.unban_window(ui.ctx());
        self.mod_log_window(ui.ctx());
        // a clicked match of the search across all chats: go there, its search set
        if let Some((key, query)) = self.global_search_window(ui.ctx()) {
            if let Some(i) = self.chat_index(&key) {
                self.chats[i].search = Some(query.to_lowercase());
            }
            act.focus = Some(key);
        }
        self.user_log_window(ui.ctx());
        if let Some((group, a)) = self.usercard_window(ui.ctx()) {
            use super::usercard::CardAction;
            match a {
                CardAction::Moderate(m) => act.moderate = Some((group, m)),
                CardAction::Mention(n) => {
                    if let Some(tab) = self.chats.iter_mut().find(|t| t.login == group) {
                        if !tab.input.is_empty() && !tab.input.ends_with(' ') {
                            tab.input.push(' ');
                        }
                        tab.input.push_str(&format!("@{n} "));
                    }
                    act.focus = Some(group);
                }
                CardAction::Hide(n) => act.hide_user = Some((n, true)),
                CardAction::OpenChannel(c) => act.open_channel = Some(c),
            }
        }
        self.popup_windows(ui.ctx(), &mut act);
        self.apply_main_actions(act);
    }

    /// Splits side by side with a 1 px gap, like Chatterino.
    fn splits_row(
        &mut self,
        ui: &mut egui::Ui,
        splits: &[String],
        host: Host,
        act: &mut MainActions,
    ) {
        let n = splits.len().max(1) as f32;
        let full = ui.available_rect_before_wrap();
        let gap = 1.0;
        let w = (full.width() - gap * (n - 1.0)) / n;
        for (c, key) in splits.iter().enumerate() {
            let x = full.left() + c as f32 * (w + gap);
            let rect = egui::Rect::from_min_size(
                egui::Pos2::new(x, full.top()),
                Vec2::new(w, full.height()),
            );
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).id_salt((
                "split-col",
                key.as_str(),
                format!("{host:?}"),
            )));
            self.split_ui(&mut child, key, host, splits.len(), c, act);
        }
        ui.allocate_rect(full, Sense::hover());
    }

    fn split_ui(
        &mut self,
        ui: &mut egui::Ui,
        key: &str,
        host: Host,
        count: usize,
        pos: usize,
        act: &mut MainActions,
    ) {
        let Some(idx) = self.chat_index(key) else {
            return;
        };
        let focused = idx == self.active_chat;
        let info = self.live_info(key);
        let options = self.send_options(key);
        let (primary, _) = tab_primary(key);
        let mut chosen = if focused {
            self.settings.send_platform
        } else {
            primary
        };
        if !options.iter().any(|(p, r)| *p == chosen && r.is_ok()) {
            if let Some((p, _)) = options.iter().find(|(_, r)| r.is_ok()) {
                chosen = *p;
            }
        }
        let notify = self
            .settings
            .notify_channels
            .iter()
            .any(|c| c.eq_ignore_ascii_case(key));
        let mut look = self.chat_look(self.chats[idx].search.clone());
        // moderation only where the account moderates this Twitch channel (or owns it); the
        // per-message buttons when Settings > Moderation turns them on or the split's sword does
        let twitch_channel = self.chats[idx].twitch.clone();
        // screenshot test only: act as a moderator everywhere
        let shot_mod = self.shot.is_some() && std::env::var("LIVE_STREAMS_SHOT_MOD").is_ok();
        let moderates_here = twitch_channel
            .as_ref()
            .is_some_and(|c| self.mod_in.contains(c))
            || shot_mod;
        let mod_mode = twitch_channel
            .as_ref()
            .is_some_and(|c| self.settings.mod_mode.contains(c));
        look.moderation = (look.moderation || shot_mod) && moderates_here;
        look.mod_buttons = look.mod_buttons || mod_mode;
        // the filters picked for this split (split menu > Set filters)
        if let Some(names) = self.settings.split_filters.get(key) {
            look.filters = names
                .iter()
                .filter_map(|n| {
                    self.matchers
                        .filters
                        .get(n)
                        .and_then(|f| f.as_ref().ok())
                        .cloned()
                })
                .collect();
        }
        let hs = &self.settings;
        // Chatterino's header text: "name (live) - uptime - viewers - game - title"
        let mut header = self.chats[idx].display.clone();
        if let Some(li) = info.first() {
            header.push_str(if info.len() > 1 {
                " (live on several)"
            } else {
                " (live)"
            });
            if hs.header_uptime {
                header.push_str(&format!(" - {}", li.uptime));
            }
            if hs.header_viewers {
                header.push_str(&format!(
                    " - {}",
                    model::format_viewers(info.iter().map(|l| l.viewers).sum())
                ));
            }
            if hs.header_game && !li.game.is_empty() {
                header.push_str(&format!(" - {}", li.game));
            }
            if hs.header_title && !li.title.is_empty() {
                header.push_str(&format!(" - {}", li.title));
            }
        }
        let id = egui::Id::unique(("split", key, format!("{host:?}")));
        let full = ui.max_rect();

        // ---- header (Dark.json splits.header): #2e2e2e, focused #444 with blue text
        let header_h = 28.0;
        let header_rect = egui::Rect::from_min_size(full.min, Vec2::new(full.width(), header_h));
        let header_resp = ui.interact(header_rect, id.with("header"), Sense::click());
        {
            let p = ui.painter();
            p.rect_filled(
                header_rect,
                0.0,
                if focused {
                    crate::style::theme::c().header_focused_bg
                } else {
                    crate::style::theme::c().header_bg
                },
            );
            let color = if focused {
                crate::style::theme::c().header_focused_text
            } else {
                crate::style::theme::c().header_text
            };
            // keep clear of the three buttons on the right (and centred, so the same on the left)
            let max_w = (header_rect.width() - 2.0 * 84.0).max(40.0);
            let mut job = egui::text::LayoutJob::single_section(
                header.clone(),
                egui::TextFormat::simple(FontId::new(13.0, crate::style::bold()), color),
            );
            job.wrap.max_width = max_w;
            job.wrap.max_rows = 1;
            job.wrap.break_anywhere = true;
            job.wrap.overflow_character = Some('…');
            let g = ui.painter().layout_job(job);
            let pos = egui::Pos2::new(
                header_rect.center().x - g.size().x / 2.0,
                header_rect.center().y - g.size().y / 2.0,
            );
            if !info.is_empty() {
                p.circle_filled(
                    egui::Pos2::new(pos.x - 8.0, header_rect.center().y),
                    3.0,
                    crate::style::theme::c().live,
                );
            }
            p.galley(pos, g, color);
        }
        // header buttons on the right
        let mut bar = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(header_rect.shrink2(Vec2::new(4.0, 3.0)))
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
        );
        bar.spacing_mut().item_spacing.x = 2.0;
        let dim = crate::style::theme::c().tab_text;
        // the split menu: the "⋯" button and a right-click on the header show the same menu,
        // with the live list's watch items (browser, player, Streamlink...)
        let menu = SplitMenu {
            key,
            host,
            pos,
            count,
            notify,
            channels: self.channels_of(key),
            filters: self
                .settings
                .filters
                .iter()
                .map(|f| {
                    (
                        f.name.clone(),
                        self.settings
                            .split_filters
                            .get(key)
                            .is_some_and(|l| l.contains(&f.name)),
                    )
                })
                .collect(),
            custom_player: (!self.settings.custom_player.trim().is_empty())
                .then(|| self.settings.custom_player.trim().to_string()),
            // a Twitch channel and a Twitch sign-in: the Moderation submenu
            mod_tools: (self.account.is_some()
                || (self.shot.is_some() && std::env::var("LIVE_STREAMS_SHOT_MOD").is_ok()))
                && self
                    .chats
                    .iter()
                    .find(|t| t.login == key)
                    .is_some_and(|t| t.twitch.is_some()),
            auto_translate: self.settings.auto_translate.iter().any(|c| c == key),
        };
        // Chatterino's header buttons, right to left: add split, the split menu, chatters
        let _ = dim;
        if crate::style::bar_icon_sized(
            &mut bar,
            crate::style::BarIcon::Plus,
            Vec2::new(24.0, 22.0),
        )
        .on_hover_text("Add split (Ctrl+T)")
        .clicked()
        {
            act.focus = Some(key.to_string());
            act.new_split = true;
        }
        let dots = crate::style::bar_icon_sized(
            &mut bar,
            crate::style::BarIcon::Dots,
            Vec2::new(22.0, 22.0),
        )
        .on_hover_text("Split menu (right-click the header too)");
        egui::Popup::menu(&dots).show(|ui| split_menu(ui, act, &menu));
        if crate::style::bar_icon_sized(
            &mut bar,
            crate::style::BarIcon::People,
            Vec2::new(24.0, 22.0),
        )
        .on_hover_text("Chatters")
        .clicked()
        {
            act.chatters = Some(key.to_string());
        }
        // Chatterino's moderation mode: a sword, only in channels you moderate
        if let (true, Some(ch)) = (look.moderation, twitch_channel.as_ref()) {
            let r = crate::style::bar_icon_toggle(
                &mut bar,
                crate::style::BarIcon::Sword,
                Vec2::new(24.0, 22.0),
                mod_mode,
            )
            .on_hover_text(if mod_mode {
                "Moderation mode is on: mod buttons in front of every message (click to turn off)"
            } else {
                "Moderation mode: mod buttons in front of every message"
            });
            if r.clicked() {
                act.toggle_mod_mode = Some(ch.clone());
            }
        }
        if header_resp.clicked() {
            act.focus = Some(key.to_string());
        }
        header_resp.context_menu(|ui| split_menu(ui, act, &menu));
        if self.shot.is_some()
            && pos + 1 == count
            && std::env::var("LIVE_STREAMS_SHOT_MENU").as_deref() == Ok("split")
        {
            // screenshot test only: the split menu shown open
            egui::Area::new(id.with("shot-split-menu"))
                .fixed_pos(egui::Pos2::new(
                    header_rect.right() - 250.0,
                    header_rect.bottom(),
                ))
                .order(egui::Order::Foreground)
                .show(ui.ctx(), |ui| {
                    egui::Frame::menu(ui.style())
                        .show(ui, |ui| split_menu(ui, &mut MainActions::default(), &menu))
                });
        }
        let show_preview = header_resp.hovered()
            || (self.shot.is_some()
                && pos == 0
                && std::env::var("LIVE_STREAMS_SHOT_MENU").as_deref() == Ok("preview"));
        if !info.is_empty() && show_preview {
            // the stream preview, centred just below the split's title
            egui::Area::new(id.with("preview"))
                .order(egui::Order::Tooltip)
                .pivot(egui::Align2::CENTER_TOP)
                .fixed_pos(egui::Pos2::new(
                    header_rect.center().x,
                    header_rect.bottom() + 4.0,
                ))
                .constrain_to(ui.ctx().content_rect())
                .interactable(false)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.set_max_width(320.0);
                        ui.vertical_centered(|ui| {
                            for li in &info {
                                if let Some(url) = &li.preview {
                                    ui.add(
                                        egui::Image::new(url.as_str())
                                            .fit_to_exact_size(Vec2::new(320.0, 180.0))
                                            .corner_radius(2),
                                    );
                                }
                                if !li.title.is_empty() {
                                    ui.label(
                                        egui::RichText::new(&li.title)
                                            .font(FontId::new(13.0, crate::style::bold())),
                                    );
                                }
                                if !li.game.is_empty() {
                                    ui.label(&li.game);
                                }
                                ui.weak(format!(
                                    "{}: live for {} with {} viewers",
                                    li.platform.label(),
                                    li.uptime,
                                    model::format_viewers(li.viewers)
                                ));
                                ui.add_space(4.0);
                            }
                        });
                    })
                });
        }

        // ---- completion (Chatterino / chatterino7 / Moltorino / Chatty): ":em" suggests
        // emotes, "@us" suggests chatters while typing; Tab completes and cycles
        let input_id = id.with("input");
        let shot_typed = std::env::var("LIVE_STREAMS_SHOT_CAROUSEL").ok();
        let shot_complete = self.shot.is_some()
            && pos == 0
            && (std::env::var("LIVE_STREAMS_SHOT_COMPLETE").is_ok() || shot_typed.is_some());
        if shot_complete && self.chats[idx].input.is_empty() {
            // screenshot test only: a half-typed @mention, or the given text
            self.chats[idx].input = shot_typed.unwrap_or_else(|| "hey @a".into());
        }
        let focused_input = shot_complete || ui.ctx().memory(|m| m.has_focus(input_id));
        let text_now = self.chats[idx].input.clone();
        let word_start = text_now
            .rfind(char::is_whitespace)
            .map(|i| i + 1)
            .unwrap_or(0);
        let word = text_now[word_start..].to_string();
        let dismissed = self.chats[idx].ac_dismissed.as_deref() == Some(word.as_str());
        let popup_items: Vec<Completion> =
            if self.settings.completion_popup && focused_input && !dismissed {
                if let Some(q) = word.strip_prefix(':').filter(|q| q.chars().count() >= 2) {
                    self.complete_emotes(idx, q, 30)
                } else if let Some(q) = word.strip_prefix('@').filter(|q| !q.is_empty()) {
                    self.complete_users(idx, q, 30, true)
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            };
        let mut accept: Option<String> = None;
        // the emote carousel: emote suggestions as a sliding row; Tab moves along it and
        // Enter takes the chosen one
        let carousel = self.settings.emote_carousel && word.starts_with(':');
        if focused_input && !popup_items.is_empty() {
            let n = popup_items.len();
            let tab = &mut self.chats[idx];
            tab.ac_sel = tab.ac_sel.min(n - 1);
            ui.ctx().input_mut(|i| {
                if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)
                    || (carousel && i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight))
                    || (carousel && n > 1 && i.consume_key(egui::Modifiers::NONE, egui::Key::Tab))
                {
                    tab.ac_sel = (tab.ac_sel + 1) % n;
                }
                // Shift+Tab: the suggestion before (Chatterino); with Shift held these keys
                // must be caught here too, or egui moves the focus to the next button and
                // the following Enter would press it (it opened the emote window)
                if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)
                    || i.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab)
                    || (carousel && i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft))
                {
                    tab.ac_sel = (tab.ac_sel + n - 1) % n;
                }
                if i.consume_key(egui::Modifiers::NONE, egui::Key::Tab)
                    || i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                    || i.consume_key(egui::Modifiers::SHIFT, egui::Key::Enter)
                {
                    accept = Some(popup_items[tab.ac_sel].insert.clone());
                }
                if i.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                    tab.ac_dismissed = Some(word.clone());
                }
            });
        } else if focused_input && self.settings.tab_completion {
            let (tab_key, back) = ui.ctx().input_mut(|i| {
                let f = i.consume_key(egui::Modifiers::NONE, egui::Key::Tab);
                let b = i.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab);
                (f || b, b)
            });
            if tab_key && !word.is_empty() {
                // cycle when the word is still the one Tab put there
                let cycling = self.chats[idx]
                    .tab_cycle
                    .as_ref()
                    .filter(|(_, _, last)| *last == word)
                    .cloned();
                let (matches, pos) = match cycling {
                    Some((m, p, _)) => {
                        let n = m.len().max(1);
                        let next = if back { (p + n - 1) % n } else { (p + 1) % n };
                        (m, next)
                    }
                    None => {
                        let (users_only, q) = match word.strip_prefix('@') {
                            Some(q) => (true, q),
                            None => (false, word.as_str()),
                        };
                        let mut m: Vec<(String, Option<String>)> = Vec::new();
                        if !users_only {
                            m.extend(
                                self.complete_emotes(idx, q, 50)
                                    .into_iter()
                                    .filter(|c| c.prefix)
                                    .map(|c| (c.insert, c.image)),
                            );
                        }
                        m.extend(
                            self.complete_users(idx, q, 50, users_only)
                                .into_iter()
                                .filter(|c| c.prefix)
                                .map(|c| (c.insert, c.image)),
                        );
                        (m, 0)
                    }
                };
                if let Some(pick) = matches.get(pos).map(|(name, _)| name.clone()) {
                    self.chats[idx].tab_cycle = Some((matches, pos, pick.clone()));
                    let mut t = text_now[..word_start].to_string();
                    t.push_str(&pick);
                    self.chats[idx].input = t;
                    set_cursor_end(ui.ctx(), input_id, &self.chats[idx].input);
                }
            }
        }
        if let Some(ins) = accept.take() {
            let mut t = text_now[..word_start].to_string();
            t.push_str(&ins);
            t.push(' ');
            let tab = &mut self.chats[idx];
            tab.input = t;
            tab.ac_sel = 0;
            set_cursor_end(ui.ctx(), input_id, &tab.input);
        }

        // ---- search bar (Ctrl+F)
        let mut top = header_rect.bottom();
        // spellcheck: the dictionary (once loaded) and which words of the message are
        // emotes, chatters or own words
        let spell_dicts = if self.settings.spellcheck {
            crate::spell::dictionaries(&self.settings.spell_languages)
        } else {
            None
        };
        let spell_dict = spell_dicts.as_ref();
        let spell_known = match spell_dict {
            Some(_) => self.spell_known_words(idx),
            None => HashSet::new(),
        };
        // the pinned message, where you moderate (Twitch's pins)
        if let Some(pin) = self.pin_of(key).filter(|_| moderates_here) {
            let r = egui::Rect::from_min_size(
                egui::Pos2::new(full.left(), top),
                Vec2::new(full.width(), 26.0),
            );
            if super::modtools::pin_bar(ui, r, &pin) {
                act.moderate = Some((
                    key.to_string(),
                    crate::moderation::ModAction {
                        user: pin.sender.clone(),
                        user_id: String::new(),
                        kind: crate::moderation::ModKind::Unpin(pin.message_id.clone()),
                    },
                ));
            }
            top = r.bottom();
        }
        let tab = &mut self.chats[idx];
        if let Some(q) = tab.search.as_mut() {
            let r = egui::Rect::from_min_size(
                egui::Pos2::new(full.left(), top),
                Vec2::new(full.width(), 26.0),
            );
            ui.painter()
                .rect_filled(r, 0.0, crate::style::theme::c().input_bg);
            let mut s = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(r.shrink2(Vec2::new(6.0, 2.0)))
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            let resp = s.add(
                egui::TextEdit::singleline(q)
                    .hint_text("Search messages")
                    .frame(egui::Frame::NONE)
                    .desired_width(r.width() - 40.0),
            );
            if s.add(egui::Button::new("✕").frame(false)).clicked()
                || resp.lost_focus() && s.input(|i| i.key_pressed(egui::Key::Escape))
            {
                act.toggle_search = Some(key.to_string());
            }
            top = r.bottom();
        }

        // ---- input (Dark.json splits.input #242424)
        let input_h = 34.0;
        let input_rect = egui::Rect::from_min_max(
            egui::Pos2::new(full.left(), full.bottom() - input_h),
            full.max,
        );
        let msg_rect = egui::Rect::from_min_max(
            egui::Pos2::new(full.left(), top),
            egui::Pos2::new(full.right(), input_rect.top()),
        );
        ui.painter()
            .rect_filled(msg_rect, 0.0, crate::style::theme::c().split_bg);
        let mut msgs = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(msg_rect.shrink2(Vec2::new(6.0, 3.0)))
                .id_salt(id.with("msgs")),
        );
        let channel = tab.twitch.clone().unwrap_or_else(|| tab.login.clone());
        match crate::style::chat_lines(&mut msgs, &tab.lines, &self.emotes, &channel, &look) {
            // right-click an emote in chat > Add to message / Add to favourites
            Some(crate::style::LineAction::Add(text)) => {
                if !tab.input.is_empty() && !tab.input.ends_with(' ') {
                    tab.input.push(' ');
                }
                tab.input.push_str(&text);
                tab.input.push(' ');
            }
            Some(crate::style::LineAction::Favourite(e)) => act.favourite = Some(e),
            Some(crate::style::LineAction::Moderate(m)) => {
                act.moderate = Some((key.to_string(), m))
            }
            Some(crate::style::LineAction::UserCard(u)) => {
                act.usercard = Some((key.to_string(), u))
            }
            Some(crate::style::LineAction::Hide(u)) => act.hide_user = Some((u, true)),
            Some(crate::style::LineAction::OpenChannel(c)) => act.open_channel = Some(c),
            // message menu > Reply: the next message answers it; the box gets the cursor
            Some(crate::style::LineAction::Reply(r)) => {
                tab.reply_to = Some(r);
                ui.ctx().memory_mut(|m| m.request_focus(input_id));
                act.focus = Some(key.to_string());
            }
            Some(crate::style::LineAction::Thread(t)) => act.thread = Some((key.to_string(), t)),
            Some(crate::style::LineAction::Translate(l)) => {
                act.translate = Some((key.to_string(), l))
            }
            Some(crate::style::LineAction::OpenUrl(u)) => crate::browser::open_url(&u),
            None => {}
        }
        // a click anywhere in the messages focuses the split. Read from the input, not with
        // a widget over the messages: one drawn on top took every click from them (names,
        // mod buttons, the message and emote menus)
        if ui.input(|i| i.pointer.any_click()) && ui.rect_contains_pointer(msg_rect) {
            act.focus = Some(key.to_string());
        }

        // Image Uploader (Chatterino's): an image pasted into this message box waits for
        // "Upload" (nothing leaves the computer before that)
        if ui.ctx().memory(|m| m.has_focus(input_id)) {
            let pasted = ui.ctx().input_mut(|i| {
                let pos = i
                    .events
                    .iter()
                    .position(|e| matches!(e, egui::Event::PasteImage(_)))?;
                match i.events.remove(pos) {
                    egui::Event::PasteImage(img) => Some(img),
                    _ => None,
                }
            });
            if let Some(img) = pasted {
                if self.settings.uploader_enabled {
                    tab.pending_image = Some(img);
                } else {
                    push_line(tab, ChatLine::System("To paste images, turn on the image uploader in Settings > External tools.".into()), &self.emotes);
                }
            }
        }
        if let Some(img) = tab.pending_image.clone() {
            let bar = egui::Rect::from_min_max(
                egui::Pos2::new(full.left(), input_rect.top() - 30.0),
                egui::Pos2::new(full.right(), input_rect.top()),
            );
            ui.painter()
                .rect_filled(bar, 0.0, crate::style::theme::c().shade(30));
            let mut b = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(bar.shrink2(Vec2::new(8.0, 3.0)))
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            let host = self
                .settings
                .uploader_url
                .split('/')
                .nth(2)
                .unwrap_or("the image host")
                .to_string();
            b.label(format!(
                "Upload the pasted image ({}×{}) to {host}?",
                img.size[0], img.size[1]
            ));
            if b.button("Upload").clicked() {
                act.upload = Some((key.to_string(), img));
                tab.pending_image = None;
            }
            if b.button("Cancel").clicked() {
                tab.pending_image = None;
            }
        }

        if self.shot.is_some()
            && pos == 0
            && tab.reply_to.is_none()
            && std::env::var("LIVE_STREAMS_SHOT_REPLY").is_ok()
        {
            // screenshot test only: writing a reply
            tab.reply_to = Some(crate::chat::Reply {
                parent_id: "x".into(),
                parent_name: "viewer_one".into(),
                parent_body: "hello chat Kappa".into(),
                thread_id: "x".into(),
            });
        }
        if let (Some(r), None) = (tab.reply_to.clone(), &tab.pending_image) {
            let bar = egui::Rect::from_min_max(
                egui::Pos2::new(full.left(), input_rect.top() - 26.0),
                egui::Pos2::new(full.right(), input_rect.top()),
            );
            let pal = crate::style::theme::c();
            ui.painter().rect_filled(bar, 0.0, pal.shade(24));
            ui.painter().vline(
                bar.left() + 1.5,
                bar.y_range(),
                egui::Stroke::new(3.0, pal.accent),
            );
            let mut b = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(bar.shrink2(Vec2::new(10.0, 2.0)))
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            let body: String = r.parent_body.chars().take(80).collect();
            if b.add(egui::Button::new("✕").frame(false))
                .on_hover_text("Don't reply (Esc)")
                .clicked()
                || (ui.ctx().memory(|m| m.has_focus(input_id))
                    && ui.input(|i| i.key_pressed(egui::Key::Escape)))
            {
                tab.reply_to = None;
            }
            b.add(
                egui::Label::new(
                    egui::RichText::new(format!("Replying to @{}: {body}", r.parent_name))
                        .color(pal.text),
                )
                .truncate(),
            );
        }
        ui.painter()
            .rect_filled(input_rect, 0.0, crate::style::theme::c().input_bg);
        let mut row = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(input_rect.shrink2(Vec2::new(6.0, 5.0)))
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        row.spacing_mut().item_spacing.x = 4.0;
        let before = chosen;
        // where the message goes: the platform's logo (and a small arrow); the list shows
        // each platform's logo and name, greyed out (with the reason) where sending isn't possible
        let pick = {
            let (rect, resp) = row.allocate_exact_size(Vec2::new(40.0, 24.0), Sense::click());
            if resp.hovered() {
                row.painter()
                    .rect_filled(rect, 2.0, crate::style::theme::c().shade(41));
            }
            let mut badge = row.new_child(egui::UiBuilder::new().max_rect(
                egui::Rect::from_center_size(
                    rect.left_center() + Vec2::new(13.0, 0.0),
                    Vec2::splat(18.0),
                ),
            ));
            crate::style::platform_badge(&mut badge, chosen, 18.0);
            let a = rect.right_center() + Vec2::new(-8.0, 0.5);
            row.painter().add(egui::Shape::convex_polygon(
                vec![
                    a + Vec2::new(-3.5, -2.0),
                    a + Vec2::new(3.5, -2.0),
                    a + Vec2::new(0.0, 2.5),
                ],
                crate::style::theme::c().tab_text,
                Stroke::NONE,
            ));
            resp.on_hover_text(format!("Sending to {} - click to choose", chosen.label()))
        };
        let mut platform_popup = egui::Popup::menu(&pick);
        if self.shot.is_some()
            && pos == 0
            && std::env::var("LIVE_STREAMS_SHOT_MENU").as_deref() == Ok("platform")
        {
            // screenshot test only: the list shown open
            platform_popup = platform_popup.open_memory(egui::SetOpenCommand::Bool(true));
        }
        platform_popup.show(|ui| {
            ui.set_max_width(170.0);
            for (p, ok) in &options {
                let r = ui
                    .add_enabled_ui(ok.is_ok(), |ui| {
                        let (rect, resp) =
                            ui.allocate_exact_size(Vec2::new(170.0, 26.0), Sense::click());
                        if chosen == *p {
                            ui.painter().rect_filled(
                                rect,
                                2.0,
                                crate::style::theme::c().accent.gamma_multiply(0.3),
                            );
                        } else if resp.hovered() {
                            ui.painter()
                                .rect_filled(rect, 2.0, crate::style::theme::c().shade(41));
                        }
                        let mut b = ui.new_child(egui::UiBuilder::new().max_rect(
                            egui::Rect::from_center_size(
                                rect.left_center() + Vec2::new(14.0, 0.0),
                                Vec2::splat(18.0),
                            ),
                        ));
                        crate::style::platform_badge(&mut b, *p, 18.0);
                        let col = if ok.is_ok() {
                            crate::style::theme::c().strong_text
                        } else {
                            crate::style::theme::c().tab_text.gamma_multiply(0.6)
                        };
                        ui.painter().text(
                            rect.left_center() + Vec2::new(30.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            p.label(),
                            FontId::proportional(13.0),
                            col,
                        );
                        resp
                    })
                    .inner;
                let r = match ok {
                    Err(why) => r.on_disabled_hover_text(why),
                    Ok(()) => r,
                };
                if r.clicked() {
                    chosen = *p;
                    ui.close();
                }
            }
        });
        if chosen != before {
            act.platform_for = Some((key.to_string(), chosen));
        }
        let can = options.iter().any(|(p, r)| *p == chosen && r.is_ok());
        let counter = if tab.input.is_empty() || !self.settings.input_counter {
            String::new()
        } else {
            format!("{}", tab.input.chars().count())
        };
        let live_count = self.streams.len();
        let right_w = 78.0 + if counter.is_empty() { 0.0 } else { 34.0 };
        if can {
            let w = (row.available_width() - right_w).max(40.0);
            let known = |w: &str| spell_known.contains(w);
            // misspelled words get a red line under them
            let mut spell_layouter =
                |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap_width: f32| {
                    let text = buf.as_str();
                    let font = egui::FontSelection::default().resolve(ui.style());
                    let color = ui
                        .visuals()
                        .override_text_color
                        .unwrap_or_else(|| ui.visuals().widgets.inactive.text_color());
                    let plain = egui::TextFormat::simple(font, color);
                    let wrong = egui::TextFormat {
                        underline: egui::Stroke::new(1.5, Color32::from_rgb(232, 72, 72)),
                        ..plain.clone()
                    };
                    let mut job = egui::text::LayoutJob::default();
                    job.wrap.max_width = wrap_width;
                    let mut at = 0;
                    if let Some(d) = spell_dict {
                        for r in crate::spell::misspelled(d, text, &known) {
                            job.append(&text[at..r.start], 0.0, plain.clone());
                            job.append(&text[r.clone()], 0.0, wrong.clone());
                            at = r.end;
                        }
                    }
                    job.append(&text[at..], 0.0, plain);
                    ui.fonts_mut(|f| f.layout_job(job))
                };
            let mut edit = egui::TextEdit::singleline(&mut tab.input)
                .id(input_id)
                .hint_text("Send message...")
                .frame(egui::Frame::NONE)
                .char_limit(chat::MAX_MESSAGE)
                // Tab / Shift+Tab are for completion: they never move the focus to the
                // buttons next to the box (a single line never gets a tab character)
                .lock_focus(true)
                .desired_width(w);
            if spell_dict.is_some() {
                edit = edit.layouter(&mut spell_layouter);
            }
            let out = edit.show(&mut row);
            let r = out.response.clone();
            if r.gained_focus() || r.clicked() {
                act.focus = Some(key.to_string());
            }
            // right-click on a misspelled word: suggestions and "Add to the dictionary"
            if let (Some(d), true) = (spell_dict, r.secondary_clicked()) {
                tab.spell_menu = None;
                if let Some(p) = r.interact_pointer_pos() {
                    let c = out.galley.cursor_from_pos(p - out.galley_pos);
                    let byte = tab
                        .input
                        .char_indices()
                        .nth(c.index.0)
                        .map_or(tab.input.len(), |(b, _)| b);
                    // the word being typed counts here too
                    let finished = format!("{} ", tab.input);
                    if let Some(at) = crate::spell::misspelled(d, &finished, &known)
                        .into_iter()
                        .find(|at| at.start <= byte && byte <= at.end)
                    {
                        let word = tab.input[at.clone()].to_string();
                        let list = crate::spell::suggestions(d, &word);
                        tab.spell_menu = Some((at, word, list));
                    }
                }
            }
            if let Some((at, word, list)) = tab.spell_menu.clone() {
                r.context_menu(|ui| {
                    if list.is_empty() {
                        ui.weak("No suggestions");
                    }
                    for s in &list {
                        if ui.button(s).clicked() {
                            if tab.input.get(at.clone()) == Some(word.as_str()) {
                                tab.input.replace_range(at.clone(), s);
                            }
                            tab.spell_menu = None;
                            ui.close();
                        }
                    }
                    ui.separator();
                    if ui
                        .button(format!("Add \"{word}\" to the dictionary"))
                        .clicked()
                    {
                        if !self.settings.spell_words.contains(&word) {
                            self.settings.spell_words.push(word.clone());
                        }
                        tab.spell_menu = None;
                        ui.close();
                    }
                });
            }
            if r.has_focus() {
                // Up/Down: previously sent messages (Chatterino)
                let (up, down) = row.input(|i| {
                    (
                        i.key_pressed(egui::Key::ArrowUp),
                        i.key_pressed(egui::Key::ArrowDown),
                    )
                });
                if up && !tab.history.is_empty() {
                    tab.history_pos = tab.history_pos.saturating_sub(1);
                    tab.input = tab.history[tab.history_pos].clone();
                }
                if down && !tab.history.is_empty() {
                    tab.history_pos = (tab.history_pos + 1).min(tab.history.len());
                    tab.input = tab
                        .history
                        .get(tab.history_pos)
                        .cloned()
                        .unwrap_or_default();
                }
            }
            if r.lost_focus() && row.input(|i| i.key_pressed(egui::Key::Enter)) {
                let text = std::mem::take(&mut tab.input);
                if !text.trim().is_empty() {
                    tab.history.push(text.clone());
                    tab.history_pos = tab.history.len();
                    act.send = Some((key.to_string(), text, chosen));
                }
                r.request_focus();
            }
        } else {
            row.weak("Read only");
            if row.link("Sign in").clicked() {
                act.settings = true;
            }
        }
        row.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if crate::style::bar_icon_sized(
                ui,
                crate::style::BarIcon::Smiley,
                Vec2::new(28.0, 24.0),
            )
            .on_hover_text("Emotes (Ctrl+E)")
            .clicked()
            {
                act.focus = Some(key.to_string());
                act.toggle_emotes = true;
            }
            // the live channels (Alt+L), next to the emote button
            let n = live_count;
            let r = crate::style::bar_icon_sized(
                ui,
                crate::style::BarIcon::Live,
                Vec2::new(26.0, 22.0),
            );
            if n > 0 {
                ui.label(egui::RichText::new(n.to_string()).size(11.5).color(dim));
            }
            if r.on_hover_text(format!("Live channels: {n} (Alt+L)"))
                .clicked()
            {
                act.toggle_live = true;
            }
            if !counter.is_empty() {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(counter).size(11.0).color(dim));
            }
        });
        let popup_at = input_rect.left_top() + Vec2::new(6.0, -2.0);
        // while Tab cycles through matches without the list: the carousel shows them all,
        // so the next picks can be seen before they are sent
        let cycle = self.chats[idx].tab_cycle.clone().filter(|(m, _, last)| {
            let input = &self.chats[idx].input;
            let cur = &input[input.rfind(char::is_whitespace).map_or(0, |i| i + 1)..];
            m.len() > 1 && m.iter().any(|(_, img)| img.is_some()) && last == cur
        });
        if let (true, true, Some((matches, pos, _))) = (
            popup_items.is_empty() && focused_input,
            self.settings.emote_carousel,
            cycle,
        ) {
            if let Some(i) = emote_carousel(
                ui.ctx(),
                id.with("tab-carousel"),
                popup_at,
                &matches,
                pos,
                "Tab: next   Shift+Tab: back",
            ) {
                let pick = matches[i].0.clone();
                let input = self.chats[idx].input.clone();
                let start = input.rfind(char::is_whitespace).map_or(0, |i| i + 1);
                self.chats[idx].input = format!("{}{pick}", &input[..start]);
                self.chats[idx].tab_cycle = Some((matches, i, pick));
                set_cursor_end(ui.ctx(), input_id, &self.chats[idx].input);
                ui.ctx().memory_mut(|m| m.request_focus(input_id));
            }
        }
        // the suggestions above the message box: the emote carousel, or a list
        if !popup_items.is_empty() && carousel {
            let sel = self.chats[idx].ac_sel.min(popup_items.len() - 1);
            let items: Vec<(String, Option<String>)> = popup_items
                .iter()
                .map(|c| (c.label.clone(), c.image.clone()))
                .collect();
            if let Some(i) = emote_carousel(
                ui.ctx(),
                id.with("completion"),
                popup_at,
                &items,
                sel,
                "Tab / arrows: choose   Enter: complete   Esc: close",
            ) {
                let mut t = text_now[..word_start].to_string();
                t.push_str(&popup_items[i].insert);
                t.push(' ');
                self.chats[idx].input = t;
                set_cursor_end(ui.ctx(), input_id, &self.chats[idx].input);
                ui.ctx().memory_mut(|m| m.request_focus(input_id));
            }
        } else if !popup_items.is_empty() {
            let sel = self.chats[idx].ac_sel.min(popup_items.len() - 1);
            let mut clicked: Option<String> = None;
            egui::Area::new(id.with("completion"))
                .order(egui::Order::Foreground)
                .pivot(egui::Align2::LEFT_BOTTOM)
                .fixed_pos(popup_at)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.set_min_width(240.0);
                        egui::ScrollArea::vertical()
                            .max_height(260.0)
                            .show(ui, |ui| {
                                for (i, c) in popup_items.iter().enumerate() {
                                    let r = ui
                                        .horizontal(|ui| {
                                            if let Some(url) = &c.image {
                                                ui.add(
                                                    egui::Image::new(url.as_str())
                                                        .fit_to_exact_size(Vec2::splat(22.0)),
                                                );
                                            }
                                            ui.add(
                                                egui::Label::new(
                                                    egui::RichText::new(&c.label).strong(),
                                                )
                                                .selectable(false),
                                            );
                                            ui.weak(&c.note);
                                        })
                                        .response;
                                    let r = ui.interact(
                                        r.rect,
                                        id.with(("completion-row", i)),
                                        Sense::click(),
                                    );
                                    if i == sel {
                                        ui.painter().rect_filled(
                                            r.rect.expand(2.0),
                                            2.0,
                                            crate::style::theme::c().accent.gamma_multiply(0.25),
                                        );
                                        r.scroll_to_me(None);
                                    }
                                    if r.clicked() {
                                        clicked = Some(c.insert.clone());
                                    }
                                }
                            });
                        ui.weak("Tab / Enter: complete   Up / Down: choose   Esc: close");
                    });
                });
            if let Some(ins) = clicked {
                let mut t = text_now[..word_start].to_string();
                t.push_str(&ins);
                t.push(' ');
                self.chats[idx].input = t;
                set_cursor_end(ui.ctx(), input_id, &self.chats[idx].input);
                ui.ctx().memory_mut(|m| m.request_focus(input_id));
            }
        }
    }

    /// A reply thread (Chatterino's reply thread popup): its first message and every reply,
    /// in the chat's look; "Reply in thread" answers the newest one.
    fn thread_window(&mut self, ctx: &egui::Context, act: &mut MainActions) {
        if self.shot.is_some()
            && self.thread.is_none()
            && std::env::var("LIVE_STREAMS_SHOT_THREAD").is_ok()
        {
            // screenshot test only: the sample reply thread
            if let Some(t) = self.chats.first() {
                self.thread = Some((t.login.clone(), crate::demo::DEMO_THREAD.into()));
            }
        }
        let Some((key, thread)) = self.thread.clone() else {
            return;
        };
        let Some(idx) = self.chat_index(&key) else {
            self.thread = None;
            return;
        };
        // the thread's messages, each with its translation when it has one
        let mut lines: std::collections::VecDeque<ChatLine> = std::collections::VecDeque::new();
        let mut after_one = false;
        for l in &self.chats[idx].lines {
            let in_thread = match l {
                ChatLine::Msg { msg_id, reply, .. } => {
                    *msg_id == thread || reply.as_ref().is_some_and(|r| r.thread_id == thread)
                }
                _ => after_one && super::translate::is_note(l),
            };
            if in_thread {
                lines.push_back(l.clone());
            }
            after_one = in_thread && matches!(l, ChatLine::Msg { .. });
        }
        let count = lines
            .iter()
            .filter(|l| matches!(l, ChatLine::Msg { .. }))
            .count();
        let look = self.chat_look(None);
        let channel = self.chats[idx]
            .twitch
            .clone()
            .unwrap_or_else(|| self.chats[idx].login.clone());
        let mut open = true;
        let mut reply_now = false;
        let mut line_act: Option<crate::style::LineAction> = None;
        egui::Window::new(format!("Reply thread · {count} messages"))
            .id(egui::Id::unique("reply-thread"))
            .open(&mut open)
            .collapsible(false)
            .default_size([460.0, 320.0])
            .show(ctx, |ui| {
                if lines.is_empty() {
                    ui.weak("The thread's messages aren't in this chat any more.");
                }
                let h = (ui.available_height() - 36.0).max(80.0);
                ui.allocate_ui(Vec2::new(ui.available_width(), h), |ui| {
                    line_act = crate::style::chat_lines(ui, &lines, &self.emotes, &channel, &look);
                });
                if ui.button("Reply in thread").clicked() {
                    reply_now = true;
                }
            });
        // the message menu works here as in the chat (Translate, Reply, Pin, Hide ...)
        use crate::style::LineAction as A;
        match line_act {
            Some(A::UserCard(u)) => act.usercard = Some((key.clone(), u)),
            Some(A::Moderate(m)) => act.moderate = Some((key.clone(), m)),
            Some(A::Translate(l)) => act.translate = Some((key.clone(), l)),
            Some(A::Hide(u)) => act.hide_user = Some((u, true)),
            Some(A::OpenChannel(c)) => act.open_channel = Some(c),
            Some(A::Favourite(e)) => act.favourite = Some(e),
            Some(A::Reply(r)) => {
                self.chats[idx].reply_to = Some(r);
                act.focus = Some(key.clone());
            }
            Some(A::Add(text)) => {
                let input = &mut self.chats[idx].input;
                if !input.is_empty() && !input.ends_with(' ') {
                    input.push(' ');
                }
                input.push_str(&text);
                input.push(' ');
                act.focus = Some(key.clone());
            }
            Some(A::OpenUrl(u)) => crate::browser::open_url(&u),
            Some(A::Thread(_)) | None => {}
        }
        if reply_now {
            // answer the newest message of the thread
            if let Some(ChatLine::Msg {
                name, text, msg_id, ..
            }) = lines
                .iter()
                .rev()
                .find(|l| matches!(l, ChatLine::Msg { .. }))
            {
                self.chats[idx].reply_to = Some(crate::chat::Reply {
                    parent_id: msg_id.clone(),
                    parent_name: name.clone(),
                    parent_body: text.clone(),
                    thread_id: thread.clone(),
                });
                act.focus = Some(key.clone());
            }
        }
        if !open {
            self.thread = None;
        }
    }

    /// Pop-out windows (Chatterino popups): each its own native window with splits.
    fn popup_windows(&mut self, ctx: &egui::Context, act: &mut MainActions) {
        let wins: Vec<(u64, TabPage)> = self.popups.clone();
        let mut closed: Vec<u64> = Vec::new();
        for (id, page) in wins {
            let title = format!("{} - {}", self.page_title(&page), crate::APP_ID);
            let mut b = egui::ViewportBuilder::default()
                .with_title(title)
                .with_inner_size([520.0, 680.0])
                .with_min_inner_size([280.0, 200.0]);
            if self.shot.is_some() {
                b = b.with_active(false);
            }
            // a pop-out reopened at start goes where it was, as big as it was
            if let Some((pos, size)) = self.popup_start.get(&id) {
                b = b.with_position(*pos).with_inner_size(*size);
            }
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of(("popup", id)),
                b,
                |ui, _| {
                    if ui.ctx().input(|i| i.viewport().close_requested()) {
                        closed.push(id);
                    }
                    let (outer, inner) = ui
                        .ctx()
                        .input(|i| (i.viewport().outer_rect, i.viewport().inner_rect));
                    // a reopened window gets centred on the main one: move it back where it
                    // was (for a few frames at most: that place may be off every screen now),
                    // and don't save the centred place meanwhile
                    let mut placing = false;
                    if let Some((pos, _)) = self.popup_start.get(&id) {
                        let pos = egui::pos2(pos[0], pos[1]);
                        let key = ui.make_persistent_id(("popup-placed", id));
                        let away = outer.is_some_and(|o| (o.min - pos).length() > 2.0);
                        let tries = ui.ctx().data_mut(|d| {
                            let n = d.get_temp_mut_or_default::<u32>(key);
                            // there once: done (moving it later is the person's doing)
                            *n = if away { n.saturating_add(1) } else { u32::MAX };
                            *n
                        });
                        if away && tries <= 30 {
                            ui.ctx()
                                .send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
                            placing = true;
                        }
                    }
                    if let (false, Some(o), Some(i)) = (placing, outer, inner) {
                        self.popup_rects
                            .insert(id, ([o.min.x, o.min.y], [i.width(), i.height()]));
                    }
                    // a pop-out is its own window: its keys don't reach the main window's hotkeys
                    self.popup_hotkeys(ui.ctx(), id);
                    egui::CentralPanel::no_frame()
                        .frame(egui::Frame::new().fill(crate::style::theme::c().window_bg))
                        .show(ui, |ui| {
                            self.splits_row(ui, &page.splits, Host::Popup(id), act);
                        });
                },
            );
        }
        for id in closed {
            if let Some((_, page)) = self.popups.iter().find(|(i, _)| *i == id).cloned() {
                for k in page.splits {
                    self.close_split(Host::Popup(id), &k);
                }
            }
        }
    }

    /// The emote window, as chatterino7's emote popup: "Emotes in #channel", a search over
    /// all emotes, and the tabs Favourite, Subs, Channel, Global and Emojis, each set under a
    /// centred title. Click: add to the message; right-click: the emote's menu.
    fn emote_menu_window(&mut self, ctx: &egui::Context, act: &mut MainActions) {
        if !self.emote_menu {
            return;
        }
        let mut open = true;
        let group = self
            .chats
            .get(self.active_chat)
            .map(|t| t.login.clone())
            .unwrap_or_default();
        let channel = self
            .chats
            .get(self.active_chat)
            .and_then(|t| t.twitch.clone())
            .unwrap_or_else(|| group.clone());
        let shown_channel = self
            .chats
            .get(self.active_chat)
            .map(|t| t.display.clone())
            .unwrap_or_default();
        let favourites: std::collections::HashSet<String> = self
            .settings
            .emote_favourites
            .iter()
            .map(|(n, _, _)| n.clone())
            .collect();
        // the sets of each tab: (title, emotes)
        let provider = |label: &str| match label {
            "FFZ" => emotes::Provider::Ffz,
            "BTTV" => emotes::Provider::Bttv,
            "7TV" => emotes::Provider::SevenTv,
            "Kick" => emotes::Provider::Kick,
            "YouTube" => emotes::Provider::YouTube,
            _ => emotes::Provider::Twitch,
        };
        let to_emotes = |list: &Vec<(String, String, String)>| -> Vec<emotes::Emote> {
            list.iter()
                .map(|(n, u, p)| emotes::Emote {
                    name: n.clone(),
                    url: u.clone(),
                    provider: provider(p),
                })
                .collect()
        };
        let mut tabs: Vec<(&str, Vec<(String, Vec<emotes::Emote>)>)> = Vec::new();
        // Favourite holds only what you added; the emotes you used are under Recent
        tabs.push((
            "Favourite",
            vec![(
                "Favourite".into(),
                to_emotes(&self.settings.emote_favourites),
            )],
        ));
        tabs.push((
            "Recent",
            vec![("Recent".into(), to_emotes(&self.settings.recent_emotes))],
        ));
        let mut kinds: Vec<String> = self
            .emotes
            .user_twitch
            .iter()
            .map(|(k, _)| k.clone())
            .collect();
        kinds.sort();
        kinds.dedup();
        let kind_list = |k: &str| -> Vec<emotes::Emote> {
            self.emotes
                .user_twitch
                .iter()
                .filter(|(kk, _)| kk == k)
                .map(|(_, e)| e.clone())
                .collect()
        };
        let sorted = |m: &emotes::EmoteMap| -> Vec<emotes::Emote> {
            let mut v: Vec<emotes::Emote> = m.values().cloned().collect();
            v.sort_by_key(|e| e.name.to_lowercase());
            v
        };
        tabs.push((
            "Subs",
            kinds
                .iter()
                .filter(|k| !k.to_lowercase().contains("global"))
                .map(|k| (k.clone(), kind_list(k)))
                .collect(),
        ));
        let mut channel_sets: Vec<(String, Vec<emotes::Emote>)> = Vec::new();
        if let Some(set) = self.emotes.channels.get(&channel) {
            channel_sets.push(("FrankerFaceZ".into(), sorted(&set.ffz)));
            channel_sets.push(("BetterTTV".into(), sorted(&set.bttv)));
            channel_sets.push(("7TV".into(), sorted(&set.seventv)));
        }
        if let Some(sets) = self.emotes.kick_sets.get(&group) {
            for (name, list) in sets {
                channel_sets.push((name.clone(), list.clone()));
            }
        }
        tabs.push(("Channel", channel_sets));
        let mut global: Vec<(String, Vec<emotes::Emote>)> = kinds
            .iter()
            .filter(|k| k.to_lowercase().contains("global"))
            .map(|k| (k.clone(), kind_list(k)))
            .collect();
        global.push(("FrankerFaceZ".into(), sorted(&self.emotes.global.ffz)));
        global.push(("BetterTTV".into(), sorted(&self.emotes.global.bttv)));
        global.push(("7TV".into(), sorted(&self.emotes.global.seventv)));
        tabs.push(("Global", global));
        tabs.push(("Emojis", Vec::new()));
        let title = if shown_channel.is_empty() {
            "Emotes".to_string()
        } else {
            format!("Emotes in #{shown_channel}")
        };
        egui::Window::new(title)
            .id(egui::Id::unique("emote-menu"))
            .open(&mut open)
            .collapsible(false)
            // centred, Chatterino's emote popup size; it can be resized
            .default_size([620.0, 500.0])
            .min_size([320.0, 260.0])
            .max_size([980.0, 840.0])
            .resizable(true)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.add(egui::TextEdit::singleline(&mut self.emote_search).hint_text("Search all emotes...").desired_width(f32::INFINITY));
                let q = self.emote_search.trim().to_lowercase();
                if q.is_empty() {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        for (i, (name, _)) in tabs.iter().enumerate() {
                            if crate::style::flat_tab(ui, name, self.emote_tab == i, crate::style::theme::c().accent).clicked() {
                                self.emote_tab = i;
                            }
                        }
                    });
                    ui.separator();
                }
                let tab = self.emote_tab.min(tabs.len() - 1);
                egui::ScrollArea::vertical().id_salt("emote-scroll").auto_shrink([false, false]).show(ui, |ui| {
                    // one set: a centred title, then its emotes centred and wrapped
                    let mut set_ui = |ui: &mut egui::Ui, title: &str, list: Vec<&emotes::Emote>| {
                        if list.is_empty() {
                            return;
                        }
                        ui.vertical_centered(|ui| ui.label(egui::RichText::new(title).strong().color(crate::style::theme::c().strong_text)));
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                            for e in list {
                                let r = crate::style::emote_button(ui, e);
                                if r.clicked() {
                                    act.picked = Some(e.clone());
                                }
                                // right-click: add, favourite, copy name, copy / open link
                                r.context_menu(|ui| match crate::style::emote_menu_items(ui, e, favourites.contains(&e.name)) {
                                    Some(crate::style::LineAction::Add(_)) => act.picked = Some(e.clone()),
                                    Some(crate::style::LineAction::Favourite(f)) => act.favourite = Some(f),
                                    Some(_) | None => {}
                                });
                            }
                        });
                        ui.add_space(8.0);
                    };
                    if !q.is_empty() {
                        // the search goes through every tab, each emote once
                        let mut seen = std::collections::HashSet::new();
                        for (_, sets) in &tabs {
                            for (title, list) in sets {
                                let hits: Vec<&emotes::Emote> = list.iter().filter(|e| e.name.to_lowercase().contains(&q) && seen.insert(e.url.clone())).collect();
                                set_ui(ui, title, hits);
                            }
                        }
                        let emoji: Vec<&(&str, &str)> = EMOJIS.iter().filter(|(_, n)| n.contains(q.as_str())).collect();
                        if !emoji.is_empty() {
                            emoji_grid(ui, &emoji, act);
                        }
                    } else if tabs[tab].0 == "Emojis" {
                        let all: Vec<&(&str, &str)> = EMOJIS.iter().collect();
                        emoji_grid(ui, &all, act);
                    } else {
                        let mut any = false;
                        for (title, list) in &tabs[tab].1 {
                            any |= !list.is_empty();
                            set_ui(ui, title, list.iter().collect());
                        }
                        if !any {
                            ui.vertical_centered(|ui| {
                                ui.weak(match tabs[tab].0 {
                                    "Favourite" => "Right-click an emote > Add to favourites.",
                                    "Recent" => "Emotes you use show up here.",
                                    "Subs" => "Sign in to Twitch to see your subscriber and follower emotes here.",
                                    "Channel" => "This channel has no 7TV, BTTV, FFZ or Kick emotes (or they are still loading).",
                                    _ => "Loading...",
                                })
                            });
                        }
                    }
                });
            });
        if !open {
            self.emote_menu = false;
        }
    }
    fn apply_main_actions(&mut self, mut act: MainActions) {
        if let Some(key) = &act.focus {
            if self.chat_index(key) != Some(self.active_chat) {
                let (p, _) = tab_primary(key);
                self.settings.send_platform = p;
            }
            self.focus_key(key);
        }
        if let Some((key, p)) = act.platform_for {
            self.focus_key(&key);
            self.settings.send_platform = p;
        }
        if let Some(p) = act.select_page {
            self.select_page(p);
        }
        if let Some((p, d)) = act.move_page {
            let q = (p as isize + d) as usize;
            if q < self.pages.len() {
                self.move_page_to(p, if d > 0 { q + 1 } else { q });
            }
        }
        if let Some((from, to)) = act.drop_page {
            self.move_page_to(from, to);
        }
        if let Some(p) = act.lock_page {
            if let Some(pg) = self.pages.get_mut(p) {
                pg.locked = !pg.locked;
            }
        }
        if let Some((p, g)) = act.set_group {
            let g = g.trim().to_string();
            if p < self.pages.len() {
                self.pages[p].group = g.clone();
                // keep a group's tabs together: next to its last other tab
                if !g.is_empty() {
                    if let Some(last) = (0..self.pages.len())
                        .rev()
                        .find(|&i| i != p && self.pages[i].group == g)
                    {
                        self.move_page_to(p, last + 1);
                    }
                }
            }
        }
        if let Some(g) = act.toggle_group {
            if let Some(i) = self.settings.collapsed_groups.iter().position(|c| *c == g) {
                self.settings.collapsed_groups.remove(i);
            } else {
                self.settings.collapsed_groups.push(g);
            }
        }
        if let Some((p, c)) = act.tab_color {
            if let Some(pg) = self.pages.get_mut(p) {
                pg.color = c;
            }
        }
        if let Some(p) = act.always_visible {
            if let Some(pg) = self.pages.get_mut(p) {
                pg.always_visible = !pg.always_visible;
            }
        }
        if act.lock_layout {
            self.settings.tabs_locked = !self.settings.tabs_locked;
        }
        if let Some((host, key, d)) = act.move_split {
            let splits = match host {
                Host::Main(p) => self.pages.get_mut(p).map(|pg| &mut pg.splits),
                Host::Popup(id) => self
                    .popups
                    .iter_mut()
                    .find(|(i, _)| *i == id)
                    .map(|(_, pg)| &mut pg.splits),
            };
            if let Some(s) = splits {
                if let Some(i) = s.iter().position(|k| *k == key) {
                    let j = (i as isize + d) as usize;
                    if j < s.len() {
                        s.swap(i, j);
                    }
                }
            }
        }
        if let Some(p) = act.rename_page {
            let current = self
                .pages
                .get(p)
                .map(|pg| self.page_title(pg))
                .unwrap_or_default();
            self.renaming = Some((p, current));
        }
        if let Some(p) = act.popup_page {
            self.popup_page(p);
        }
        if let Some(k) = act.popup_split {
            self.popup_split(&k);
        }
        if let Some(p) = act.close_page {
            self.close_page(p);
        }
        if let Some((host, key)) = act.close_split {
            self.close_split(host, &key);
        }
        if let Some(k) = act.change_channel {
            self.change_channel = Some(k);
            self.open_dialog = Some((Platform::Twitch, String::new()));
        }
        if let Some(k) = act.clear {
            if let Some(t) = self.chats.iter_mut().find(|t| t.login == k) {
                t.lines.clear();
            }
        }
        if let Some(k) = act.toggle_search {
            if let Some(t) = self.chats.iter_mut().find(|t| t.login == k) {
                t.search = if t.search.is_some() {
                    None
                } else {
                    Some(String::new())
                };
            }
        }
        if let Some(k) = act.toggle_notify {
            let list = &mut self.settings.notify_channels;
            if let Some(i) = list.iter().position(|c| c.eq_ignore_ascii_case(&k)) {
                list.remove(i);
            } else {
                list.push(k);
            }
        }
        if act.new_split {
            self.open_split_next = true;
            self.open_dialog = Some((Platform::Twitch, String::new()));
        }
        if act.new_tab {
            self.new_empty_tab();
        }
        // Moltorino's local commands: handled here and never sent to any chat
        if let Some((ch, text, _)) = act.send.as_ref() {
            if let Some(cmd) = local_command(text) {
                let ch = ch.clone();
                act.send = None;
                match cmd {
                    LocalCmd::Hide(u) => act.hide_user = Some((u, true)),
                    LocalCmd::Unhide(u) => act.hide_user = Some((u, false)),
                    LocalCmd::Chatters => act.chatters = Some(ch),
                    LocalCmd::Export => act.export = Some(ch),
                    LocalCmd::Join(c) => act.open_channel = Some(c),
                    LocalCmd::Part => act.leave = Some(ch),
                    LocalCmd::Usage(u) => self.push_to(&ch, ChatLine::System(u)),
                    LocalCmd::ModLogs(u) => self.open_mod_log(&ch, u.unwrap_or_default()),
                    LocalCmd::UnbanRequests => self.open_unban_requests(&ch),
                }
            }
        }
        // Twitch mod / broadcaster commands (/timeout, /slow, /poll ...): through Twitch's API
        if let Some((ch, text, Platform::Twitch)) = act.send.as_ref() {
            if let Some(parsed) = crate::moderation::parse_command(text) {
                let ch = ch.clone();
                act.send = None;
                match parsed {
                    Ok(cmd) => self.mod_command(&ch, cmd),
                    Err(usage) => self.push_to(&ch, ChatLine::System(usage)),
                }
            }
        }
        if let Some((ch, cmd)) = act.mod_command.take() {
            self.mod_command(&ch, cmd);
        }
        if let Some(ch) = act.mod_help.take() {
            self.push_to(&ch, ChatLine::System(crate::moderation::HELP.to_string()));
        }
        if let Some((user, hide)) = act.hide_user.take() {
            let low = user.to_lowercase();
            let list = &mut self.settings.hidden_users;
            let had = list.iter().any(|u| u.eq_ignore_ascii_case(&low));
            list.retain(|u| !u.eq_ignore_ascii_case(&low));
            if hide {
                list.push(low.clone());
            }
            let note = match (hide, had) {
                (true, _) => format!("Messages from {user} are hidden here (/unhide {user} shows them again). They aren't blocked or banned."),
                (false, true) => format!("Messages from {user} are shown again."),
                (false, false) => format!("{user} wasn't hidden."),
            };
            if let Some(k) = self.chats.get(self.active_chat).map(|t| t.login.clone()) {
                self.push_to(&k, ChatLine::System(note));
            }
        }
        if let Some(k) = act.leave.take() {
            self.leave_channel(&k);
        }
        if let Some(k) = act.export.take() {
            self.export_chat(&k);
        }
        if let Some(name) = act.open_channel.take() {
            self.open_channel_link(&name);
        }
        if let Some((ch, text, platform)) = act.send {
            if self.demo {
                let name = self
                    .account
                    .as_ref()
                    .map(|a| a.login.clone())
                    .unwrap_or_else(|| "you".into());
                let text = self.expand_command(&text, &tab_primary(&ch).1);
                self.push_to(
                    &ch,
                    ChatLine::Msg {
                        time: chat::clock(),
                        name,
                        color: None,
                        text,
                        action: false,
                        own: true,
                        emotes: Vec::new(),
                        badges: Vec::new(),
                        segs: Vec::new(),
                        platform,
                        msg_id: String::new(),
                        user_id: String::new(),
                        reply: None,
                    },
                );
            } else if !self.run_plugin_command(&ch, &text, platform) {
                // not a plugin command (Settings > Plugins): send it
                self.send_message(ch, text, platform);
            }
        }
        if let Some(k) = act.streamlink {
            self.open_in_streamlink(&k);
        }
        if let Some(g) = act.platforms {
            let l = self.settings.links.get(&g).cloned().unwrap_or_default();
            self.links_edit = Some((g, l));
        }
        if act.settings {
            self.open_settings(Page::General);
        }
        if act.toggle_live {
            self.live_open = !self.live_open;
        }
        if let Some(r) = act.row {
            self.run_action(r);
        }
        if let Some(who) = act.switch_account {
            match who {
                Some(name) => {
                    self.settings.current_twitch = name;
                    self.settings.account = AccountMode::Device;
                }
                None => self.settings.account = AccountMode::Off,
            }
            self.load_account();
        }
        if let Some((key, img)) = act.upload {
            let cfg = super::features::Uploader {
                url: self.settings.uploader_url.clone(),
                field: self.settings.uploader_field.clone(),
                headers: self.settings.uploader_headers.clone(),
                link: self.settings.uploader_link.clone(),
                delete: self.settings.uploader_delete.clone(),
            };
            self.push_to(&key, ChatLine::System("Uploading the image...".into()));
            let (tx, ctx) = (self.side_tx.clone(), self.ctx.clone());
            std::thread::spawn(move || {
                let r = super::features::png_of(&img)
                    .and_then(|png| super::features::upload_image(&cfg, &png));
                let _ = tx.send(SideMsg::Uploaded(key, r));
                ctx.request_repaint();
            });
        }
        if let Some((key, m)) = act.moderate {
            self.moderate(&key, m);
        }
        if let Some(t) = act.thread {
            self.thread = Some(t);
        }
        if let Some((key, user)) = act.usercard {
            self.open_usercard(&key, user);
        }
        if let Some((key, line)) = act.translate {
            self.translate_line(&key, &line, true);
        }
        if let Some(key) = act.toggle_translate {
            self.toggle_auto_translate(&key);
        }
        if let Some(ch) = act.toggle_mod_mode {
            if let Some(i) = self.settings.mod_mode.iter().position(|c| *c == ch) {
                self.settings.mod_mode.remove(i);
            } else {
                self.settings.mod_mode.push(ch);
            }
        }
        if let Some(v) = act.set_tab_visibility {
            self.settings.tab_live_only = v == 1;
            self.settings.tabs_hidden = v == 2;
        }
        if let Some(l) = act.set_tab_layout {
            self.settings.appearance.tab_position = l;
        }
        if let Some(e) = act.emoji {
            if let Some(tab) = self.chats.get_mut(self.active_chat) {
                tab.input.push_str(&e);
            }
        }
        if let Some(e) = act.favourite {
            let favs = &mut self.settings.emote_favourites;
            if let Some(i) = favs.iter().position(|(n, _, _)| *n == e.name) {
                favs.remove(i);
            } else {
                favs.push((
                    e.name.clone(),
                    e.url.clone(),
                    e.provider.label().to_string(),
                ));
            }
        }
        if let Some((k, name)) = act.toggle_filter {
            let list = self.settings.split_filters.entry(k).or_default();
            if let Some(i) = list.iter().position(|n| *n == name) {
                list.remove(i);
            } else {
                list.push(name);
            }
        }
        if let Some(k) = act.chatters {
            self.chatters = if self.chatters.as_ref().is_some_and(|(c, _)| *c == k) {
                None
            } else {
                Some((k, String::new()))
            };
        }
        if act.accounts_page {
            self.open_settings(Page::Accounts);
        }
        if act.toggle_emotes {
            self.emote_menu = !self.emote_menu;
        }
        if let Some(e) = act.picked {
            if let Some(tab) = self.chats.get_mut(self.active_chat) {
                if !tab.input.is_empty() && !tab.input.ends_with(' ') {
                    tab.input.push(' ');
                }
                tab.input.push_str(&e.name);
                tab.input.push(' ');
            }
            self.remember_emote(&e);
            if self.settings.close_menu_on_pick {
                self.emote_menu = false;
            }
        }
    }

    // ------------------------------------------------------------ live window

    /// The Live Streams list as its own window (Alt+L), as Chatty's Live Channels dialog.
    pub(super) fn live_window(&mut self, ctx: &egui::Context) {
        if !self.live_open {
            return;
        }
        let title = self.title();
        let mut builder = egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size([700.0, 820.0])
            .with_min_inner_size([360.0, 240.0]);
        if self.shot.is_some() {
            builder = builder.with_active(false);
        }
        let mut close = false;
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("live-streams"),
            builder,
            |ui, _class| {
                if ui.ctx().input(|i| i.viewport().close_requested()) {
                    close = true;
                }
                if ui
                    .ctx()
                    .input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::L))
                {
                    close = true;
                }
                egui::CentralPanel::no_frame()
                    .frame(egui::Frame::new().fill(BG))
                    .show(ui, |ui| self.list_ui(ui));
            },
        );
        if close {
            self.live_open = false;
        }
    }
}

/// One completion suggestion.
struct Completion {
    label: String,
    /// What replaces the typed word.
    insert: String,
    note: String,
    image: Option<String>,
    /// The suggestion starts with what was typed (Tab completion uses only these).
    prefix: bool,
}

/// Where a dragged tab would go: the index (in `places`, the shown tabs in order) of the tab
/// it lands in front of, or None for the end. Tabs at the top or bottom can wrap onto more
/// rows, so a row counts first: above a tab's row is before it, and within its row so is the
/// left half of it. In a column (tabs on the left or right) only the height counts.
fn drop_before(places: &[(usize, egui::Rect)], pos: egui::Pos2, vertical: bool) -> Option<usize> {
    places.iter().position(|(_, r)| {
        if vertical {
            pos.y < r.center().y
        } else {
            pos.y < r.top() || (pos.y <= r.bottom() && pos.x < r.center().x)
        }
    })
}

/// Which suggestions the emote carousel shows: all of them when they fit (7), else a window
/// of 7 with the chosen one in the middle, wrapping around the ends.
fn carousel_window(n: usize, sel: usize) -> Vec<usize> {
    const SHOWN: usize = 7;
    if n <= SHOWN {
        return (0..n).collect();
    }
    let start = (sel + n - SHOWN / 2) % n;
    (0..SHOWN).map(|k| (start + k) % n).collect()
}

/// The emote carousel (Moltorino): suggestions as a row of pictures above the message box,
/// the chosen one bigger and highlighted, names underneath. Returns a clicked one's index.
fn emote_carousel(
    ctx: &egui::Context,
    id: egui::Id,
    at: egui::Pos2,
    items: &[(String, Option<String>)],
    sel: usize,
    hint: &str,
) -> Option<usize> {
    if items.is_empty() {
        return None;
    }
    let sel = sel.min(items.len() - 1);
    let theme = crate::style::theme::c();
    let mut clicked = None;
    egui::Area::new(id)
        .order(egui::Order::Foreground)
        .pivot(egui::Align2::LEFT_BOTTOM)
        .fixed_pos(at)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for i in carousel_window(items.len(), sel) {
                        let (name, image) = &items[i];
                        let chosen = i == sel;
                        let (rect, r) =
                            ui.allocate_exact_size(Vec2::new(68.0, 66.0), Sense::click());
                        if chosen {
                            ui.painter()
                                .rect_filled(rect, 6.0, theme.accent.gamma_multiply(0.28));
                        } else if r.hovered() {
                            ui.painter().rect_filled(
                                rect,
                                6.0,
                                ui.visuals().widgets.hovered.weak_bg_fill,
                            );
                        }
                        let picture = Rect::from_center_size(
                            rect.center() - Vec2::new(0.0, 7.0),
                            Vec2::new(60.0, if chosen { 42.0 } else { 30.0 }),
                        );
                        match image {
                            Some(url) => {
                                let img = egui::Image::new(url.as_str()).tint(if chosen {
                                    Color32::WHITE
                                } else {
                                    Color32::from_white_alpha(150)
                                });
                                // keep the emote's shape: as big as fits, wide ones stay wide
                                let size = img
                                    .load_for_size(ui.ctx(), picture.size())
                                    .ok()
                                    .and_then(|t| t.size())
                                    .filter(|s| s.x > 0.0 && s.y > 0.0)
                                    .unwrap_or(Vec2::splat(picture.height()));
                                let scale =
                                    (picture.width() / size.x).min(picture.height() / size.y);
                                img.paint_at(
                                    ui,
                                    Rect::from_center_size(picture.center(), size * scale),
                                );
                            }
                            None => {
                                ui.painter().text(
                                    picture.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "@",
                                    egui::FontId::proportional(if chosen { 24.0 } else { 18.0 }),
                                    theme.system,
                                );
                            }
                        }
                        let short: String = if name.chars().count() > 9 {
                            name.chars().take(8).chain(['…']).collect()
                        } else {
                            name.clone()
                        };
                        ui.painter().text(
                            rect.center_bottom() - Vec2::new(0.0, 4.0),
                            egui::Align2::CENTER_BOTTOM,
                            short,
                            egui::FontId::proportional(11.0),
                            if chosen {
                                theme.strong_text
                            } else {
                                theme.system
                            },
                        );
                        if r.on_hover_text(name.as_str()).clicked() {
                            clicked = Some(i);
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(&items[sel].0)
                            .strong()
                            .color(theme.strong_text),
                    );
                    ui.weak(format!("{} / {}   {hint}", sel + 1, items.len()));
                });
            });
        });
    clicked
}

/// Puts the text cursor at the end of a message box after it was filled in.
fn set_cursor_end(ctx: &egui::Context, id: egui::Id, text: &str) {
    if let Some(mut st) = egui::text_edit::TextEditState::load(ctx, id) {
        let n = text.chars().count();
        st.cursor.set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(n),
        )));
        st.store(ctx, id);
    }
}

impl LiveApp {
    /// Spellcheck: the words of a chat's message that are emotes there, chatters' names or
    /// own words (as typed, so the spellcheck can look them up).
    fn spell_known_words(&self, idx: usize) -> HashSet<String> {
        let tab = &self.chats[idx];
        // nothing typed: nothing to look up (this runs every frame for every split)
        if tab.input.trim().is_empty() {
            return HashSet::new();
        }
        let own: HashSet<String> = self
            .settings
            .spell_words
            .iter()
            .map(|w| w.to_lowercase())
            .collect();
        let chatters: HashSet<String> = tab
            .lines
            .iter()
            .filter_map(|l| match l {
                ChatLine::Msg { name, .. } => Some(name.to_lowercase()),
                _ => None,
            })
            .collect();
        let channel = tab.twitch.clone().unwrap_or_else(|| tab.login.clone());
        let set = self.emotes.channels.get(&channel);
        let g = &self.emotes.global;
        let mut known = HashSet::new();
        for w in crate::spell::word_forms(&tab.input) {
            let lw = w.to_lowercase();
            let in_map = |m: &emotes::EmoteMap| m.contains_key(w);
            let emote = set
                .is_some_and(|s| in_map(&s.seventv) || in_map(&s.bttv) || in_map(&s.ffz))
                || in_map(&g.seventv)
                || in_map(&g.bttv)
                || in_map(&g.ffz)
                || self.emotes.user_twitch.iter().any(|(_, e)| e.name == w)
                || self.emotes.kick_sets.get(&tab.login).is_some_and(|sets| {
                    sets.iter()
                        .any(|(_, list)| list.iter().any(|e| e.name == w))
                });
            if emote || own.contains(&lw) || chatters.contains(&lw) {
                known.insert(w.to_string());
            }
        }
        known
    }

    /// Emotes for completion: the account's Twitch emotes, the channel's 7TV / BTTV / FFZ
    /// and Kick emotes, then the global ones; names starting with the text first.
    fn complete_emotes(&self, idx: usize, q: &str, max: usize) -> Vec<Completion> {
        let tab = &self.chats[idx];
        let channel = tab.twitch.clone().unwrap_or_else(|| tab.login.clone());
        let mut all: Vec<&emotes::Emote> = Vec::new();
        all.extend(self.emotes.user_twitch.iter().map(|(_, e)| e));
        if let Some(set) = self.emotes.channels.get(&channel) {
            all.extend(
                set.seventv
                    .values()
                    .chain(set.bttv.values())
                    .chain(set.ffz.values()),
            );
        }
        if let Some(sets) = self.emotes.kick_sets.get(&tab.login) {
            for (_, list) in sets {
                all.extend(list.iter());
            }
        }
        let g = &self.emotes.global;
        all.extend(
            g.seventv
                .values()
                .chain(g.bttv.values())
                .chain(g.ffz.values()),
        );
        let ql = q.to_lowercase();
        let mut seen = HashSet::new();
        let mut starts: Vec<Completion> = Vec::new();
        let mut contains: Vec<Completion> = Vec::new();
        for e in all {
            let nl = e.name.to_lowercase();
            if !seen.insert(e.name.clone()) {
                continue;
            }
            let c = |prefix| Completion {
                label: e.name.clone(),
                insert: e.name.clone(),
                note: e.provider.label().to_string(),
                image: Some(e.url.clone()),
                prefix,
            };
            if nl.starts_with(&ql) {
                starts.push(c(true));
            } else if nl.contains(&ql) {
                contains.push(c(false));
            }
        }
        starts.sort_by_key(|c| c.label.len());
        starts.extend(contains);
        starts.truncate(max);
        starts
    }

    /// Chatters for completion: who wrote in this chat, most recent first.
    fn complete_users(&self, idx: usize, q: &str, max: usize, at: bool) -> Vec<Completion> {
        let ql = q.to_lowercase();
        let mut seen = HashSet::new();
        let mut starts = Vec::new();
        let mut contains = Vec::new();
        for line in self.chats[idx].lines.iter().rev() {
            if let ChatLine::Msg { name, platform, .. } = line {
                let nl = name.to_lowercase();
                if !seen.insert(nl.clone()) {
                    continue;
                }
                let c = |prefix| Completion {
                    label: name.clone(),
                    insert: if at { format!("@{name}") } else { name.clone() },
                    note: platform.label().to_string(),
                    image: None,
                    prefix,
                };
                if nl.starts_with(&ql) {
                    starts.push(c(true));
                } else if nl.contains(&ql) {
                    contains.push(c(false));
                }
            }
        }
        starts.extend(contains);
        starts.truncate(max);
        starts
    }
}

/// Emoji for the emote window's Emojis tab: (emoji, search words).
const EMOJIS: [(&str, &str); 96] = [
    ("😀", "grinning smile"),
    ("😂", "joy laugh tears"),
    ("🤣", "rofl laugh"),
    ("😊", "blush smile"),
    ("😍", "heart eyes love"),
    ("😘", "kiss"),
    ("😎", "cool sunglasses"),
    ("🤔", "thinking hmm"),
    ("😐", "neutral"),
    ("🙄", "eye roll"),
    ("😏", "smirk"),
    ("😮", "wow open mouth"),
    ("😴", "sleep"),
    ("😛", "tongue"),
    ("😉", "wink"),
    ("😢", "cry sad"),
    ("😭", "sob cry"),
    ("😡", "angry rage"),
    ("🤬", "swear"),
    ("😱", "scream fear"),
    ("😳", "flushed"),
    ("🥺", "pleading"),
    ("🤯", "mind blown"),
    ("🥳", "party"),
    ("😇", "angel innocent"),
    ("🤡", "clown"),
    ("💀", "skull dead"),
    ("👻", "ghost"),
    ("👽", "alien"),
    ("🤖", "robot"),
    ("😺", "cat smile"),
    ("🙈", "see no evil monkey"),
    ("🙉", "hear no evil"),
    ("🙊", "speak no evil"),
    ("💩", "poop"),
    ("👍", "thumbs up yes"),
    ("👎", "thumbs down no"),
    ("👏", "clap"),
    ("🙌", "raise hands"),
    ("🙏", "pray please thanks"),
    ("👋", "wave hi bye"),
    ("🤝", "handshake"),
    ("✌", "peace victory"),
    ("🤞", "fingers crossed luck"),
    ("👌", "ok"),
    ("👀", "eyes look"),
    ("💪", "strong muscle"),
    ("🫡", "salute"),
    ("🤷", "shrug"),
    ("🤦", "facepalm"),
    ("❤", "heart red love"),
    ("🧡", "orange heart"),
    ("💛", "yellow heart"),
    ("💚", "green heart"),
    ("💙", "blue heart"),
    ("💜", "purple heart"),
    ("🖤", "black heart"),
    ("🤍", "white heart"),
    ("💔", "broken heart"),
    ("💯", "hundred"),
    ("🔥", "fire lit"),
    ("✨", "sparkles"),
    ("⭐", "star"),
    ("🌟", "glowing star"),
    ("⚡", "lightning zap"),
    ("💥", "boom"),
    ("🎉", "tada party"),
    ("🎊", "confetti"),
    ("🎁", "gift present"),
    ("🏆", "trophy win"),
    ("🎮", "game controller"),
    ("🕹", "joystick"),
    ("🎵", "music note"),
    ("🎶", "notes music"),
    ("🎤", "mic sing"),
    ("📺", "tv"),
    ("📢", "loudspeaker"),
    ("🔔", "bell"),
    ("💰", "money bag"),
    ("💎", "gem diamond"),
    ("🍕", "pizza"),
    ("🍔", "burger"),
    ("🍟", "fries"),
    ("🍿", "popcorn"),
    ("☕", "coffee"),
    ("🍺", "beer"),
    ("🍷", "wine"),
    ("🎂", "cake birthday"),
    ("🍪", "cookie"),
    ("🍓", "strawberry"),
    ("🐶", "dog"),
    ("🐱", "cat"),
    ("🐸", "frog"),
    ("🐍", "snake"),
    ("🦀", "crab"),
    ("✅", "check yes"),
];

/// The Emojis tab: big emoji, click to add.
fn emoji_grid(ui: &mut egui::Ui, list: &[&(&str, &str)], act: &mut MainActions) {
    ui.vertical_centered(|ui| ui.label(egui::RichText::new("Emojis").strong()));
    ui.with_layout(
        egui::Layout::left_to_right(egui::Align::Center)
            .with_main_wrap(true)
            .with_main_align(egui::Align::Center),
        |ui| {
            for (e, words) in list {
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new(*e).size(22.0))
                            .frame(false)
                            .min_size(Vec2::new(32.0, 32.0)),
                    )
                    .on_hover_text(*words)
                    .clicked()
                {
                    act.emoji = Some(e.to_string());
                }
            }
        },
    );
}

/// What a split's menu needs.
struct SplitMenu<'a> {
    key: &'a str,
    host: Host,
    pos: usize,
    count: usize,
    notify: bool,
    channels: Vec<(Platform, String)>,
    /// Every filter, and whether this split uses it.
    filters: Vec<(String, bool)>,
    custom_player: Option<String>,
    mod_tools: bool,
    /// Auto-translate is on for this split.
    auto_translate: bool,
}

/// The menus' look: roomier rows, a soft rounded accent highlight instead of a box.
pub(super) fn menu_look(ui: &mut egui::Ui) {
    let accent = crate::style::theme::c().accent;
    let s = ui.style_mut();
    s.spacing.button_padding = Vec2::new(8.0, 5.0);
    s.spacing.item_spacing.y = 1.0;
    let v = &mut s.visuals.widgets;
    for w in [&mut v.hovered, &mut v.active, &mut v.open] {
        w.weak_bg_fill = accent.gamma_multiply(0.25);
        w.bg_fill = accent.gamma_multiply(0.25);
        w.bg_stroke = Stroke::NONE;
        w.corner_radius = 5.into();
        w.expansion = 0.0;
    }
}

/// A menu entry's contents: a fixed-width icon in the accent colour, then the label (also
/// for submenu buttons, so everything lines up).
pub(super) fn menu_atoms<'a>(
    icon: &'a str,
    label: impl Into<egui::RichText>,
) -> (egui::Atom<'a>, egui::Atom<'a>) {
    use egui::AtomExt;
    (
        egui::RichText::new(icon)
            .color(crate::style::theme::c().accent)
            .atom_size(Vec2::new(20.0, 16.0)),
        egui::Atom::from(label.into()),
    )
}

/// A menu entry: icon, label, and the shortcut dim on the right.
pub(super) fn menu_entry<'a>(
    icon: &'a str,
    label: impl Into<egui::RichText>,
    shortcut: &'a str,
) -> egui::Button<'a> {
    let c = crate::style::theme::c();
    let b = egui::Button::new(menu_atoms(icon, label));
    if shortcut.is_empty() {
        b
    } else {
        b.shortcut_text(egui::RichText::new(shortcut).small().color(c.system))
    }
}

/// An on/off menu entry: a check on the right while it's on.
fn menu_toggle<'a>(icon: &'a str, label: &'a str, on: bool) -> egui::Button<'a> {
    menu_entry(icon, label, if on { "✔" } else { "" })
}

/// A small section title in a menu.
pub(super) fn menu_heading(ui: &mut egui::Ui, title: &str) {
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(title)
            .small()
            .strong()
            .color(crate::style::theme::c().system),
    );
}

/// The split menu (Chatterino's split menu, with the live list's watch items).
fn split_menu(ui: &mut egui::Ui, act: &mut MainActions, m: &SplitMenu) {
    menu_look(ui);
    ui.set_min_width(250.0);
    let key = m.key;
    menu_heading(ui, "SPLIT");
    if ui
        .add(menu_entry("🔀", "Change channel", "Ctrl+R"))
        .clicked()
    {
        act.change_channel = Some(key.to_string());
        ui.close();
    }
    if ui.add(menu_entry("⬈", "Pop out", "Ctrl+N")).clicked() {
        act.popup_split = Some(key.to_string());
        ui.close();
    }
    if ui
        .add_enabled(m.pos > 0, menu_entry("◀", "Move left", ""))
        .clicked()
    {
        act.move_split = Some((m.host, key.to_string(), -1));
        ui.close();
    }
    if ui
        .add_enabled(m.pos + 1 < m.count, menu_entry("▶", "Move right", ""))
        .clicked()
    {
        act.move_split = Some((m.host, key.to_string(), 1));
        ui.close();
    }
    ui.separator();
    menu_heading(ui, "WATCH");
    super::watch_items(ui, &mut act.row, &m.channels, key, key);
    // External tools > Custom stream player: the scheme before the stream's address
    if let (Some(scheme), Some((p, n))) = (&m.custom_player, m.channels.first()) {
        if ui.button("Open in custom player").clicked() {
            act.row = Some(RowAction::OpenUrl(format!(
                "{scheme}{}",
                super::watch_urls(*p, n).0
            )));
            ui.close();
        }
    }
    ui.separator();
    menu_heading(ui, "CHAT");
    if ui.add(menu_entry("🔍", "Search", "Ctrl+F")).clicked() {
        act.toggle_search = Some(key.to_string());
        ui.close();
    }
    // Filters (Settings > Filters): which ones this split uses
    ui.menu_button(menu_atoms("⚙", "Set filters"), |ui| {
        menu_look(ui);
        if m.filters.is_empty() {
            ui.weak("No filters yet: Settings > Filters");
        }
        for (name, on) in &m.filters {
            let mut v = *on;
            if ui.checkbox(&mut v, name).changed() {
                act.toggle_filter = Some((key.to_string(), name.clone()));
            }
        }
    });
    if ui.add(menu_entry("🧹", "Clear messages", "")).clicked() {
        act.clear = Some(key.to_string());
        ui.close();
    }
    if m.mod_tools {
        // Chatterino's moderation tools: chat modes and clearing (you must moderate the channel)
        ui.menu_button(menu_atoms("🛡", "Moderation"), |ui| {
            use crate::moderation::ModCommand as C;
            menu_look(ui);
            ui.set_min_width(210.0);
            let items: [(&str, C); 12] = [
                ("Slow mode (30s)", C::Slow(Some(30))),
                ("Slow mode off", C::Slow(None)),
                ("Followers-only (10m)", C::Followers(Some(10))),
                ("Followers-only off", C::Followers(None)),
                ("Emote-only", C::EmoteOnly(true)),
                ("Emote-only off", C::EmoteOnly(false)),
                ("Subscribers-only", C::SubOnly(true)),
                ("Subscribers-only off", C::SubOnly(false)),
                ("Unique chat", C::UniqueChat(true)),
                ("Unique chat off", C::UniqueChat(false)),
                ("Shield mode", C::Shield(true)),
                ("Shield mode off", C::Shield(false)),
            ];
            for (i, (label, cmd)) in items.into_iter().enumerate() {
                if i > 0 && i % 2 == 0 {
                    ui.separator();
                }
                if ui.button(label).clicked() {
                    act.mod_command = Some((key.to_string(), cmd));
                    ui.close();
                }
            }
            ui.separator();
            if ui
                .button("Clear chat")
                .on_hover_text("Removes every message in the channel for everyone")
                .clicked()
            {
                act.mod_command = Some((key.to_string(), C::Clear));
                ui.close();
            }
            if ui
                .button("Mod commands...")
                .on_hover_text("Lists /timeout, /ban, /poll, /raid and the rest")
                .clicked()
            {
                act.mod_help = Some(key.to_string());
                ui.close();
            }
            ui.separator();
            // the two windows open like their commands (local, never sent)
            for (label, cmd) in [
                ("Unban requests...", "/unbanrequests"),
                ("Mod log...", "/modlogs"),
            ] {
                if ui.button(label).clicked() {
                    act.send = Some((key.to_string(), cmd.to_string(), Platform::Twitch));
                    ui.close();
                }
            }
        });
    }
    if ui
        .add(menu_entry("💾", "Export chat (text + JSON)", ""))
        .on_hover_text("Saves this chat to %APPDATA%\\Yaperino\\Exports (also /export)")
        .clicked()
    {
        act.export = Some(key.to_string());
        ui.close();
    }
    ui.separator();
    menu_heading(ui, "OPTIONS");
    if ui
        .add(menu_toggle("🔔", "Notify when live", m.notify))
        .clicked()
    {
        act.toggle_notify = Some(key.to_string());
        ui.close();
    }
    // Moltorino: translate this chat's new messages (Settings > General > Translation)
    if ui
        .add(menu_toggle(
            "🌐",
            "Auto-translate this chat",
            m.auto_translate,
        ))
        .on_hover_text("New messages in other languages get a translation under them")
        .clicked()
    {
        act.toggle_translate = Some(key.to_string());
        ui.close();
    }
    ui.separator();
    let close = egui::RichText::new("Close split").color(Color32::from_rgb(0xe8, 0x5c, 0x5c));
    if ui.add(menu_entry("✕", close, "Ctrl+W")).clicked() {
        act.close_split = Some((m.host, key.to_string()));
        ui.close();
    }
}

/// An empty tab (Chatterino's "+"): a button to add its first split. True when clicked.
fn empty_tab_hint(ui: &mut egui::Ui) -> bool {
    let r = ui.available_rect_before_wrap();
    let dim = crate::style::theme::c().tab_text;
    // double-click anywhere in the empty tab also adds a split
    let area = ui.interact(r, ui.scope_id().with("empty-tab"), Sense::click());
    ui.painter().text(
        r.center() - Vec2::new(0.0, 34.0),
        egui::Align2::CENTER_CENTER,
        "This tab is empty",
        FontId::proportional(16.0),
        crate::style::theme::c().text,
    );
    let mut add = area.double_clicked();
    let b = egui::Rect::from_center_size(r.center() + Vec2::new(0.0, 4.0), Vec2::new(150.0, 30.0));
    add |= ui.put(b, egui::Button::new("+  Add a split")).clicked();
    ui.painter().text(
        r.center() + Vec2::new(0.0, 38.0),
        egui::Align2::CENTER_CENTER,
        "or press Ctrl+T, or open a channel from the live list (Alt+L)",
        FontId::proportional(12.0),
        dim,
    );
    add
}

fn empty_hint(ui: &mut egui::Ui) {
    let r = ui.available_rect_before_wrap();
    let dim = Color32::from_rgb(0x88, 0x88, 0x88);
    ui.painter().text(
        r.center() - Vec2::new(0.0, 12.0),
        egui::Align2::CENTER_CENTER,
        "No chats open",
        FontId::proportional(16.0),
        crate::style::theme::c().text,
    );
    ui.painter().text(
        r.center() + Vec2::new(0.0, 12.0),
        egui::Align2::CENTER_CENTER,
        "Ctrl+Shift+T new tab  ·  Ctrl+T new split  ·  Alt+L live channels",
        FontId::proportional(12.0),
        dim,
    );
}

/// A local command (Moltorino): handled in the app, never sent.
#[derive(Debug, PartialEq)]
enum LocalCmd {
    Hide(String),
    Unhide(String),
    Chatters,
    Export,
    /// /join <channel>: switch to it, or open it in a new tab.
    Join(String),
    /// /part, /leave: leave this channel.
    Part,
    /// How to use it (a command given without its user).
    Usage(String),
    /// /modlogs [user]: the mod log, filtered by a user.
    ModLogs(Option<String>),
    /// /unbanrequests: the unban requests window.
    UnbanRequests,
}

fn local_command(text: &str) -> Option<LocalCmd> {
    let mut words = text.split_whitespace();
    let cmd = words.next()?.to_lowercase();
    let user = words
        .next()
        .map(|u| u.trim_start_matches('@').to_string())
        .filter(|u| !u.is_empty());
    Some(match (cmd.as_str(), user) {
        ("/hide", Some(u)) => LocalCmd::Hide(u),
        ("/unhide", Some(u)) => LocalCmd::Unhide(u),
        ("/hide" | "/unhide", None) => LocalCmd::Usage(format!("Usage: {cmd} <user>")),
        ("/chatters", _) => LocalCmd::Chatters,
        ("/join", Some(u)) => LocalCmd::Join(u.trim_start_matches('#').to_string()),
        ("/join", None) => LocalCmd::Usage("Usage: /join <channel>".into()),
        ("/part" | "/leave", _) => LocalCmd::Part,
        ("/modhelp", _) => LocalCmd::Usage(crate::moderation::HELP.to_string()),
        ("/export", _) => LocalCmd::Export,
        ("/modlogs" | "/modlog", u) => LocalCmd::ModLogs(u),
        ("/unbanrequests" | "/unbans", _) => LocalCmd::UnbanRequests,
        _ => return None,
    })
}

/// What the tab's right-click menu needs to know.
struct TabMenu {
    p: usize,
    count: usize,
    vertical: bool,
    /// The tab can't be moved or closed (it, or the layout, is locked).
    locked: bool,
    page_locked: bool,
    layout_locked: bool,
    live_only: bool,
    hidden: bool,
    position: String,
    color: Option<[u8; 3]>,
    always_visible: bool,
    /// The tab's group and all groups there are.
    group: String,
    groups: Vec<String>,
}

/// Moltorino custom tab colours.
const TAB_COLORS: [(&str, [u8; 3]); 8] = [
    ("Red", [0xe5, 0x48, 0x4d]),
    ("Orange", [0xf0, 0x8c, 0x2e]),
    ("Yellow", [0xe8, 0xc5, 0x3a]),
    ("Green", [0x4c, 0xbb, 0x5f]),
    ("Teal", [0x2e, 0xb8, 0xb0]),
    ("Blue", [0x3d, 0x8b, 0xf0]),
    ("Purple", [0x9b, 0x5d, 0xe5]),
    ("Pink", [0xe5, 0x5d, 0xb0]),
];

/// Chatterino's tab menu: rename, pop out, move, lock, tab visibility and layout, close.
fn tab_menu(ui: &mut egui::Ui, act: &mut MainActions, m: &TabMenu) {
    let p = m.p;
    ui.set_min_width(190.0);
    if ui.button("Rename tab").clicked() {
        act.rename_page = Some(p);
        ui.close();
    }
    if ui
        .add_enabled(!m.locked, egui::Button::new("Pop out tab   Ctrl+Shift+N"))
        .clicked()
    {
        act.popup_page = Some(p);
        ui.close();
    }
    let (before, after) = if m.vertical {
        ("Move up", "Move down")
    } else {
        ("Move left", "Move right")
    };
    if ui
        .add_enabled(!m.locked && p > 0, egui::Button::new(before))
        .clicked()
    {
        act.move_page = Some((p, -1));
        ui.close();
    }
    if ui
        .add_enabled(!m.locked && p + 1 < m.count, egui::Button::new(after))
        .clicked()
    {
        act.move_page = Some((p, 1));
        ui.close();
    }
    ui.separator();
    if ui
        .add_enabled(
            !m.layout_locked,
            egui::Button::new(if m.page_locked {
                "Unlock tab"
            } else {
                "Lock tab"
            }),
        )
        .clicked()
    {
        act.lock_page = Some(p);
        ui.close();
    }
    if ui
        .button(if m.layout_locked {
            "Unlock tab layout"
        } else {
            "Lock tab layout"
        })
        .clicked()
    {
        act.lock_layout = true;
        ui.close();
    }
    // Moltorino tab groups: a chip in the tab bar collapses / expands a group
    ui.menu_button("Move to group", |ui| {
        ui.set_min_width(200.0);
        for g in &m.groups {
            if ui.radio(m.group == *g, g.as_str()).clicked() {
                act.set_group = Some((p, g.clone()));
                ui.close();
            }
        }
        if ui.radio(m.group.is_empty(), "No group").clicked() {
            act.set_group = Some((p, String::new()));
            ui.close();
        }
        ui.separator();
        let id = ui.make_persistent_id("new-group-name");
        let mut name: String = ui.data(|d| d.get_temp(id)).unwrap_or_default();
        ui.horizontal(|ui| {
            let r = ui.add(
                egui::TextEdit::singleline(&mut name)
                    .hint_text("New group...")
                    .desired_width(120.0),
            );
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (ui.button("Create").clicked() || enter) && !name.trim().is_empty() {
                act.set_group = Some((p, name.trim().to_string()));
                name.clear();
                ui.close();
            }
        });
        ui.data_mut(|d| d.insert_temp(id, name));
    });
    // Moltorino: custom tab colour and "Always visible"
    ui.menu_button("Tab color", |ui| {
        if ui.radio(m.color.is_none(), "None").clicked() {
            act.tab_color = Some((p, None));
            ui.close();
        }
        for (name, c) in TAB_COLORS {
            let text = egui::RichText::new(format!("■  {name}"))
                .color(Color32::from_rgb(c[0], c[1], c[2]));
            if ui.radio(m.color == Some(c), text).clicked() {
                act.tab_color = Some((p, Some(c)));
                ui.close();
            }
        }
    });
    if ui
        .checkbox(&mut m.always_visible.clone(), "Always visible")
        .on_hover_text("Stays shown when only live tabs are shown")
        .clicked()
    {
        act.always_visible = Some(p);
        ui.close();
    }
    ui.separator();
    // Chatterino's notebook menu: tab visibility and tab layout
    let (live_only, hidden) = (m.live_only, m.hidden);
    ui.menu_button("Tab visibility", |ui| {
        for (i, label, on) in [
            (0u8, "Show all tabs", !live_only && !hidden),
            (1, "Only show live tabs", live_only && !hidden),
            (2, "Hide all tabs   Ctrl+U", hidden),
        ] {
            if ui.radio(on, label).clicked() {
                act.set_tab_visibility = Some(i);
                ui.close();
            }
        }
    });
    ui.menu_button("Tab layout", |ui| {
        for pos in ["Top", "Left", "Right", "Bottom"] {
            if ui.radio(m.position == pos, pos).clicked() {
                act.set_tab_layout = Some(pos.to_string());
                ui.close();
            }
        }
    });
    ui.separator();
    if ui
        .add_enabled(!m.locked, egui::Button::new("Close tab   Ctrl+Shift+W"))
        .clicked()
    {
        act.close_page = Some(p);
        ui.close();
    }
}
/// Move `v[from]` so it lands before index `to` (counted before the move) and return
/// where the item at `shown` ended up.
fn move_item<T>(v: &mut Vec<T>, from: usize, to: usize, shown: usize) -> usize {
    let to = to.min(v.len());
    if from >= v.len() || to == from || to == from + 1 {
        return shown;
    }
    let item = v.remove(from);
    let at = if to > from { to - 1 } else { to };
    v.insert(at, item);
    if shown == from {
        at
    } else if from < shown && at >= shown {
        shown - 1
    } else if from > shown && at <= shown {
        shown + 1
    } else {
        shown
    }
}

#[cfg(test)]
mod tab_move_tests {
    use super::{carousel_window, drop_before, local_command, move_item, LocalCmd};

    #[test]
    fn dropping_a_tab_into_wrapped_rows() {
        use egui::{pos2, Rect};
        // two rows: tabs 0, 1, 2 on the first (y 0..20), tabs 3, 4 on the second (y 21..41)
        let tab = |i: usize, x: f32, y: f32| {
            (i, Rect::from_min_max(pos2(x, y), pos2(x + 80.0, y + 20.0)))
        };
        let places = vec![
            tab(0, 0.0, 0.0),
            tab(1, 81.0, 0.0),
            tab(2, 162.0, 0.0),
            tab(3, 0.0, 21.0),
            tab(4, 81.0, 21.0),
        ];
        // first row, left half of tab 1
        assert_eq!(drop_before(&places, pos2(100.0, 10.0), false), Some(1));
        // first row, right of the last tab there: before the first tab of the next row
        assert_eq!(drop_before(&places, pos2(240.0, 10.0), false), Some(3));
        // second row, left half of tab 4 (it used to only look at x and pick tab 1)
        assert_eq!(drop_before(&places, pos2(100.0, 30.0), false), Some(4));
        // second row, after its last tab: the end
        assert_eq!(drop_before(&places, pos2(240.0, 30.0), false), None);
        // a column: only the height counts
        let column = vec![
            (0, Rect::from_min_max(pos2(0.0, 0.0), pos2(170.0, 20.0))),
            (1, Rect::from_min_max(pos2(0.0, 21.0), pos2(170.0, 41.0))),
        ];
        assert_eq!(drop_before(&column, pos2(150.0, 25.0), true), Some(1));
        assert_eq!(drop_before(&column, pos2(5.0, 50.0), true), None);
    }

    #[test]
    fn local_commands_are_never_sent() {
        assert_eq!(
            local_command("/hide @Some_One"),
            Some(LocalCmd::Hide("Some_One".into()))
        );
        assert_eq!(
            local_command("/UNHIDE someone"),
            Some(LocalCmd::Unhide("someone".into()))
        );
        assert!(matches!(local_command("/hide"), Some(LocalCmd::Usage(_))));
        assert_eq!(local_command("/chatters"), Some(LocalCmd::Chatters));
        assert_eq!(local_command("/modlogs"), Some(LocalCmd::ModLogs(None)));
        assert_eq!(
            local_command("/modlogs @Someone"),
            Some(LocalCmd::ModLogs(Some("Someone".into())))
        );
        assert_eq!(
            local_command("/unbanrequests"),
            Some(LocalCmd::UnbanRequests)
        );
        assert_eq!(local_command("/export"), Some(LocalCmd::Export));
        assert_eq!(
            local_command("/join #SomeOne"),
            Some(LocalCmd::Join("SomeOne".into()))
        );
        assert_eq!(local_command("/part"), Some(LocalCmd::Part));
        assert_eq!(local_command("/leave"), Some(LocalCmd::Part));
        assert_eq!(local_command("/me waves"), None);
        assert_eq!(local_command("hello /hide x"), None);
    }

    #[test]
    fn dragging_tabs_keeps_the_shown_one() {
        let mut v = vec!['a', 'b', 'c', 'd'];
        // drag a to the end; c was shown
        assert_eq!(move_item(&mut v, 0, 4, 2), 1);
        assert_eq!(v, ['b', 'c', 'd', 'a']);
        // drag a (shown) to the front
        assert_eq!(move_item(&mut v, 3, 0, 3), 0);
        assert_eq!(v, ['a', 'b', 'c', 'd']);
        // drag d before b; b was shown
        assert_eq!(move_item(&mut v, 3, 1, 1), 2);
        assert_eq!(v, ['a', 'd', 'b', 'c']);
        // dropping on its own place changes nothing
        assert_eq!(move_item(&mut v, 1, 2, 0), 0);
        assert_eq!(v, ['a', 'd', 'b', 'c']);
    }

    #[test]
    fn the_emote_carousel_keeps_the_chosen_one_in_the_middle() {
        // few suggestions: all of them, in order
        assert_eq!(carousel_window(3, 2), [0, 1, 2]);
        // many: 7 around the chosen one, wrapping around the ends
        assert_eq!(carousel_window(20, 10), [7, 8, 9, 10, 11, 12, 13]);
        assert_eq!(carousel_window(20, 0), [17, 18, 19, 0, 1, 2, 3]);
        assert_eq!(carousel_window(20, 19), [16, 17, 18, 19, 0, 1, 2]);
        assert_eq!(carousel_window(0, 0), Vec::<usize>::new());
    }
}
