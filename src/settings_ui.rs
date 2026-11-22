//! The Settings window, laid out like Chatterino's SettingsDialog: a search box and a
//! page list on the left, the page on the right. Pages follow Chatterino's order
//! (General, Accounts, Nicknames, Commands, Highlights, Ignores, Hotkeys, Live
//! Notifications, External tools, About) plus this app's Live list and Platforms.
//! A child of `app`, so it works on the window's own state.

use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Page {
    #[default]
    General,
    LiveList,
    Platforms,
    Accounts,
    Nicknames,
    Commands,
    Highlights,
    Ignores,
    Filters,
    Hotkeys,
    Moderation,
    Notifications,
    ExternalTools,
    Plugins,
    About,
}

impl Page {
    /// chatterino7's order and groups (SettingsDialog::addTabs): General | Accounts,
    /// Nicknames | Commands, Highlights, Ignores, Filters | Hotkeys, Moderation, Live
    /// Notifications, External tools, Plugins | About at the bottom; this app's own pages
    /// (Live list, Platforms) follow General, where Moltorino puts its own page.
    const ALL: [Page; 15] = [
        Page::General,
        Page::LiveList,
        Page::Platforms,
        Page::Accounts,
        Page::Nicknames,
        Page::Commands,
        Page::Highlights,
        Page::Ignores,
        Page::Filters,
        Page::Hotkeys,
        Page::Moderation,
        Page::Notifications,
        Page::ExternalTools,
        Page::Plugins,
        Page::About,
    ];
    /// A 16 px gap after these (chatterino7's groups).
    fn group_end(self) -> bool {
        matches!(self, Page::Platforms | Page::Nicknames | Page::Filters)
    }
    fn icon(self) -> &'static str {
        match self {
            Page::General => "⚙",
            Page::LiveList => "📺",
            Page::Platforms => "🔗",
            Page::Accounts => "👤",
            Page::Nicknames => "🏷",
            Page::Commands => "💬",
            Page::Highlights => "❗",
            Page::Ignores => "🚫",
            Page::Filters => "🔎",
            Page::Hotkeys => "⌨",
            Page::Moderation => "🛡",
            Page::Notifications => "🔔",
            Page::ExternalTools => "🔧",
            Page::Plugins => "🔌",
            Page::About => "ℹ",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Page::General => "General",
            Page::Accounts => "Accounts",
            Page::Nicknames => "Nicknames",
            Page::Commands => "Commands",
            Page::Highlights => "Highlights",
            Page::Ignores => "Ignores",
            Page::Filters => "Filters",
            Page::Hotkeys => "Hotkeys",
            Page::Moderation => "Moderation",
            Page::Notifications => "Live Notifications",
            Page::LiveList => "Live list",
            Page::Platforms => "Platforms",
            Page::ExternalTools => "External tools",
            Page::Plugins => "Plugins",
            Page::About => "About",
        }
    }
    /// Words "Find in settings" matches for this page.
    fn keywords(self) -> &'static str {
        match self {
            Page::General => "interface theme dark black light white accent colour color font family zoom size scale tab layout position top left right bottom visibility live tabs close button title bar window always on top start with windows tray minimize hide chat messages platform badge bold names completion autocomplete tab complete suggestions carousel message limit counter live dot timestamps alternate background separator badges mention emotes emote size 7tv bttv ffz recent split header viewers uptime title game restore history robotty collapse repeated link links preview previews spellcheck spelling spell dictionary dictionaries language languages typo translation translate deepl google auto-translate",
            Page::Accounts => "twitch kick youtube tiktok login sign in account device client id oauth token google",
            Page::Nicknames => "nickname rename user name",
            Page::Commands => "command alias /",
            Page::Highlights => "highlight phrase user badge blacklist colour color mention regex case sensitive flash taskbar sound",
            Page::Ignores => "ignore block hide phrase user regex replacement",
            Page::Filters => "filter filters split hide show expression",
            Page::Hotkeys => "keyboard shortcut hotkey keybinding ctrl alt",
            Page::Moderation => "moderation logs logging log files timestamp",
            Page::Notifications => "notify notification live flash taskbar sound startup selected channels",
            Page::LiveList => "login follows followed where from live streams list sort viewers uptime favorites favourites refresh interval sections platform chatty",
            Page::Platforms => "added channels kick youtube tiktok links platforms",
            Page::ExternalTools => "streamlink player quality external custom player uri scheme",
            Page::Plugins => "plugins plugin lua scripts commands",
            Page::About => "about version install licenses open source backup backups restore import settings",
        }
    }
}
/// One stretch of a settings page for "Find in settings": its page, sub-tab and heading, and
/// every word drawn there. Read once from this file's own text, so each label, hint and
/// tooltip can be found without a word list to keep up to date.
struct Spot {
    page: Page,
    sub_tab: usize,
    heading: &'static str,
    words: Vec<String>,
}

/// The functions that draw a settings page (or a section of General).
fn drawn_on(f: &str) -> Option<(Page, &'static str)> {
    Some(match f {
        "interface_section" => (Page::General, "Interface"),
        "chat_page" => (Page::General, "Chat"),
        "translation_section" => (Page::General, "Translation"),
        "emotes_page" => (Page::General, "Emotes"),
        "accounts_page" => (Page::Accounts, ""),
        "nicknames_page" => (Page::Nicknames, ""),
        "commands_page" => (Page::Commands, ""),
        "highlights_page" => (Page::Highlights, ""),
        "ignores_page" => (Page::Ignores, ""),
        "filters_page" => (Page::Filters, ""),
        "hotkeys_page" => (Page::Hotkeys, ""),
        "moderation_page" => (Page::Moderation, ""),
        "notifications_page" => (Page::Notifications, ""),
        "live_list_page" | "list_source" => (Page::LiveList, ""),
        "channels_page" => (Page::Platforms, ""),
        "streamlink_page" => (Page::ExternalTools, ""),
        "plugins_page" => (Page::Plugins, ""),
        "about_page" => (Page::About, ""),
        _ => return None,
    })
}

fn words_of(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
}

/// The string literals on a line of code, up to a `//` comment.
fn literals(line: &'static str) -> Vec<&'static str> {
    let b = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'"' {
            let start = i + 1;
            i += 1;
            while i < b.len() && b[i] != b'"' {
                i += if b[i] == b'\\' { 2 } else { 1 };
            }
            out.push(&line[start..i.min(b.len())]);
        } else if b[i] == b'/' && b.get(i + 1) == Some(&b'/') {
            break;
        }
        i += 1;
    }
    out
}

/// Every page's spots, in the order they're drawn: a new one at each page function,
/// `heading(ui, "...")` and `// search-tab: N name` (a sub-tab of Highlights or Ignores).
fn spots() -> &'static [Spot] {
    static SPOTS: std::sync::OnceLock<Vec<Spot>> = std::sync::OnceLock::new();
    SPOTS.get_or_init(|| {
        let src: &'static str = include_str!("settings_ui.rs");
        // up to the tests (at a line's start: this line names them too)
        let src = src.split("\n#[cfg(test)]").next().unwrap_or(src);
        let mut spots: Vec<Spot> = Vec::new();
        let mut page: Option<Page> = None;
        let mut in_sub_tabs = false;
        for line in src.lines() {
            let t = line.trim_start();
            let f = ["fn ", "pub fn ", "pub(super) fn ", "pub(crate) fn "]
                .iter()
                .find_map(|p| t.strip_prefix(p));
            if let Some(f) = f {
                let here = drawn_on(f.split('(').next().unwrap_or(""));
                page = here.map(|(p, _)| p);
                if let Some((p, section)) = here {
                    spots.push(Spot {
                        page: p,
                        sub_tab: 0,
                        heading: section,
                        words: words_of(section).collect(),
                    });
                }
                continue;
            }
            let Some(p) = page else { continue };
            if let Some(m) = t.strip_prefix("// search-tab: ") {
                let (n, name) = m.split_once(' ').unwrap_or((m, ""));
                let sub_tab = n.parse().unwrap_or(0);
                spots.push(Spot {
                    page: p,
                    sub_tab,
                    heading: "",
                    words: words_of(name).collect(),
                });
                continue;
            }
            // the sub-tab names belong to their tabs (the markers above), not to the first
            if t.contains("sub_tabs(") {
                in_sub_tabs = true;
            }
            if in_sub_tabs {
                in_sub_tabs = !t.contains(");");
                continue;
            }
            let lits = literals(t);
            if t.contains("heading(ui, ") {
                if let Some(h) = lits.first() {
                    let sub_tab = spots.last().map_or(0, |s| s.sub_tab);
                    spots.push(Spot {
                        page: p,
                        sub_tab,
                        heading: h,
                        words: Vec::new(),
                    });
                }
            }
            if let Some(s) = spots.last_mut() {
                for l in lits {
                    s.words.extend(words_of(l));
                }
            }
        }
        // Hotkeys lists its actions from hotkeys.rs
        if let Some(s) = spots.iter_mut().find(|s| s.page == Page::Hotkeys) {
            for (_, category, name, _) in crate::hotkeys::ACTIONS {
                s.words.extend(words_of(category).chain(words_of(name)));
            }
        }
        spots
    })
}

/// Every word typed starts a word of `words` (in any order; "lock" isn't found in "block").
fn has_words(words: &[String], query: &[String]) -> bool {
    query
        .iter()
        .all(|q| words.iter().any(|w| w.starts_with(q.as_str())))
}

/// "Find in settings": every word typed starts a word of the page: its name, its keywords
/// or anything drawn on it.
fn page_matches(p: Page, query: &str) -> bool {
    let query: Vec<String> = words_of(query).collect();
    if query.is_empty() {
        return true;
    }
    let mut words: Vec<String> = words_of(p.label()).chain(words_of(p.keywords())).collect();
    for s in spots().iter().filter(|s| s.page == p) {
        words.extend(s.words.iter().cloned());
    }
    has_words(&words, &query)
}

/// Where a search lands on a page: the first spot that has every word typed.
fn first_spot(p: Page, query: &str) -> Option<&'static Spot> {
    let query: Vec<String> = words_of(query).collect();
    if query.is_empty() {
        return None;
    }
    spots()
        .iter()
        .find(|s| s.page == p && has_words(&s.words, &query))
}

/// The headings on the shown page that match the search (drawn highlighted), and the one
/// to scroll to once.
#[derive(Default)]
struct Found {
    headings: Vec<&'static str>,
    scroll: Option<&'static str>,
}
thread_local! {
    static FOUND: std::cell::RefCell<Found> = std::cell::RefCell::new(Found::default());
}

/// A heading or section title the search found: an accent tint behind it, and scrolled to
/// when it's where the search landed.
fn mark_found(ui: &egui::Ui, text: &'static str, bg: egui::layers::ShapeIdx, r: &egui::Response) {
    FOUND.with(|f| {
        let mut f = f.borrow_mut();
        if f.headings.contains(&text) {
            ui.painter().set(
                bg,
                egui::Shape::rect_filled(
                    r.rect.expand2(Vec2::new(6.0, 2.0)),
                    4.0,
                    crate::style::theme::c().accent.gamma_multiply(0.25),
                ),
            );
        }
        if f.scroll == Some(text).filter(|t| !t.is_empty()) {
            r.scroll_to_me(Some(egui::Align::TOP));
            f.scroll = None;
        }
    });
}

/// General's sections, in the index on the right.
const GENERAL_SECTIONS: [&str; 4] = ["Interface", "Chat", "Translation", "Emotes"];

/// What the Settings window is editing right now (not saved).
#[derive(Default)]
pub struct SettingsUi {
    pub open: bool,
    pub page: Page,
    pub search: String,
    /// The search the page was last chosen for (a new one lands where it's found).
    pub last_search: String,
    pub yt_client_id: String,
    pub yt_client_secret: String,
    pub yt_busy: bool,
    pub yt_message: String,
    pub add_channel: String,
    pub streamlink_version: Option<String>,
    pub kick_client_id: String,
    pub kick_client_secret: String,
    pub kick_busy: bool,
    pub kick_message: String,
    /// Text being typed into the list pages (nicknames, commands, highlights, ignores, notify).
    pub new_a: String,
    pub new_b: String,
    /// The settings when the window opened (Cancel goes back to them).
    pub backup: Option<Box<Settings>>,
    /// About: the logo picture, whether Yaperino is installed, the last install result.
    pub logo: Option<egui::TextureHandle>,
    pub installed: Option<bool>,
    pub install_message: String,
    /// Settings backup: the last result, and a file to import.
    pub backup_message: String,
    pub backup_import: String,
    /// Spellcheck > Add language: the search text.
    pub lang_search: String,
    /// About > Open source software: rows whose license is shown.
    pub shown_licenses: Vec<usize>,
    /// The sub-tab of pages with tabs (Highlights, Ignores, Live Notifications).
    pub sub_tab: usize,
    /// General: the section to scroll to (from the index on the right).
    pub scroll_to: Option<&'static str>,
    /// Hotkeys: the search text, and the action whose new keys are being recorded.
    pub hotkey_search: String,
    pub recording: Option<&'static str>,
    /// Moderation: the logs' size (asked when shown).
    pub log_size: Option<u64>,
    pub start_message: String,
}

/// A section title like Chatterino's settings pages: larger, regular weight, with space above.
fn heading(ui: &mut egui::Ui, text: &'static str, _color: Color32) {
    ui.add_space(10.0);
    let bg = ui.painter().add(egui::Shape::Noop);
    let r = ui.label(
        egui::RichText::new(text)
            .size(15.0)
            .color(crate::style::theme::c().strong_text),
    );
    mark_found(ui, text, bg, &r);
    ui.add_space(2.0);
}

impl LiveApp {
    pub(super) fn open_settings(&mut self, page: Page) {
        self.settings_ui.open = true;
        self.settings_ui.page = page;
    }

    /// The Settings window, like Moltorino's SettingsDialog: its own window (915 x 600),
    /// "Find in settings..." and a 150 px column of pages with icons on the left (About at
    /// the bottom), the page on the right, Ok / Cancel at the bottom right.
    pub(super) fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings_ui.open {
            return;
        }
        if self.settings_ui.backup.is_none() {
            self.settings_ui.backup = Some(Box::new(self.settings.clone()));
        }
        let mut done: Option<bool> = None;
        if self.shot.is_some() {
            // screenshot mode: drawn inside the main window, so the picture shows it
            egui::Window::new("Settings")
                .id(egui::Id::unique("settings-window"))
                .title_bar(false)
                .fixed_size([915.0, 600.0])
                .pivot(egui::Align2::CENTER_CENTER)
                .fixed_pos(ctx.content_rect().center())
                .frame(egui::Frame::window(&ctx.global_style()).inner_margin(0))
                .show(ctx, |ui| {
                    ui.set_min_size(Vec2::new(915.0, 600.0));
                    done = self.settings_body(ui);
                });
        } else {
            let builder = egui::ViewportBuilder::default()
                .with_title("Yaperino Settings")
                .with_icon(crate::icon())
                .with_inner_size([915.0, 600.0])
                .with_min_inner_size([640.0, 420.0]);
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("settings"),
                builder,
                |ui, _| {
                    if ui.ctx().input(|i| i.viewport().close_requested()) {
                        // closing the window keeps what was changed (as Ok)
                        done = Some(true);
                    }
                    if ui
                        .ctx()
                        .input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
                    {
                        done = Some(false);
                    }
                    egui::CentralPanel::no_frame()
                        .frame(egui::Frame::new().fill(crate::style::theme::c().window_bg))
                        .show(ui, |ui| {
                            if let Some(d) = self.settings_body(ui) {
                                done = Some(d);
                            }
                        });
                },
            );
        }
        if let Some(ok) = done {
            self.close_settings(ok);
        }
    }

    /// Ok keeps the changes; Cancel puts the settings back as they were when the window
    /// opened, except sign-ins made meanwhile (accounts, Kick, YouTube) and what was found.
    fn close_settings(&mut self, ok: bool) {
        self.settings_ui.open = false;
        let backup = self.settings_ui.backup.take();
        if ok {
            return;
        }
        if let Some(b) = backup {
            let now = std::mem::replace(&mut self.settings, *b);
            self.settings.account = now.account;
            self.settings.twitch_accounts = now.twitch_accounts;
            self.settings.current_twitch = now.current_twitch;
            self.settings.kick = now.kick;
            self.settings.youtube = now.youtube;
            self.settings.matched = now.matched;
            self.settings.follows = now.follows;
            self.settings.links = now.links;
            self.emotes.off = self.settings.emotes_off;
            self.resort();
        }
    }

    /// The window's contents; Some(true) = Ok, Some(false) = Cancel.
    fn settings_body(&mut self, ui: &mut egui::Ui) -> Option<bool> {
        let mut done = None;
        egui::Panel::bottom("settings-buttons")
            .frame(
                egui::Frame::new()
                    .fill(crate::style::theme::c().window_bg)
                    .inner_margin(egui::Margin::symmetric(12, 8)),
            )
            .show_separator_line(true)
            .show(ui, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(egui::Button::new("Cancel").min_size(Vec2::new(76.0, 24.0)))
                        .clicked()
                    {
                        done = Some(false);
                    }
                    if ui
                        .add(egui::Button::new("Ok").min_size(Vec2::new(76.0, 24.0)))
                        .clicked()
                    {
                        done = Some(true);
                    }
                });
            });
        // "Find in settings... (Ctrl+F)" across the top, as in Chatterino
        egui::Panel::top("settings-search")
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(crate::style::theme::c().window_bg)
                    .inner_margin(egui::Margin {
                        left: 12,
                        right: 12,
                        top: 10,
                        bottom: 4,
                    }),
            )
            .show(ui, |ui| {
                let r = ui.add(
                    egui::TextEdit::singleline(&mut self.settings_ui.search)
                        .hint_text("Find in settings... (Ctrl+F)")
                        .desired_width(f32::INFINITY),
                );
                if ui.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::F)) {
                    r.request_focus();
                }
            });
        let q = self.settings_ui.search.trim().to_lowercase();
        if q != self.settings_ui.last_search {
            // the search changed: go where it's found (the open page if it's there)
            self.settings_ui.last_search = q.clone();
            if !page_matches(self.settings_ui.page, &q) {
                if let Some(p) = Page::ALL.into_iter().find(|p| page_matches(*p, &q)) {
                    self.settings_ui.page = p;
                    self.settings_ui.sub_tab = 0;
                    self.settings_ui.log_size = None;
                }
            }
            let spot = first_spot(self.settings_ui.page, &q);
            if let Some(s) = spot.filter(|s| s.sub_tab != 0) {
                self.settings_ui.sub_tab = s.sub_tab;
            }
            FOUND.with(|f| f.borrow_mut().scroll = spot.map(|s| s.heading));
        }
        // the headings of the shown page (and sub-tab) the search found
        let query: Vec<String> = words_of(&q).collect();
        let (page, sub_tab) = (self.settings_ui.page, self.settings_ui.sub_tab);
        let found: Vec<&'static str> = spots()
            .iter()
            .filter(|s| {
                s.page == page
                    && !s.heading.is_empty()
                    && (s.sub_tab == sub_tab || !matches!(page, Page::Highlights | Page::Ignores))
                    && !query.is_empty()
                    && has_words(&s.words, &query)
            })
            .map(|s| s.heading)
            .collect();
        FOUND.with(|f| f.borrow_mut().headings = found);
        let nothing = !q.is_empty() && !Page::ALL.into_iter().any(|p| page_matches(p, &q));
        egui::Panel::left("settings-tabs")
            .exact_size(150.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(crate::style::theme::c().window_bg)
                    .inner_margin(egui::Margin {
                        left: 0,
                        right: 0,
                        top: 10,
                        bottom: 10,
                    }),
            )
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.add_space(8.0);
                let hit = |p: Page| page_matches(p, &q);
                for p in Page::ALL {
                    if p == Page::About || !hit(p) {
                        continue;
                    }
                    if self.settings_tab(ui, p) && self.settings_ui.page != p {
                        self.settings_ui.page = p;
                        self.settings_ui.sub_tab = 0;
                        self.settings_ui.log_size = None;
                    }
                    // chatterino7 groups its pages with 16 px gaps
                    if p.group_end() && q.is_empty() {
                        ui.add_space(16.0);
                    }
                }
                if hit(Page::About) {
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                        if self.settings_tab(ui, Page::About) {
                            self.settings_ui.page = Page::About;
                        }
                    });
                }
            });
        // General: chatterino7's section index on the right (click to scroll there)
        if self.settings_ui.page == Page::General {
            egui::Panel::right("settings-index")
                .exact_size(150.0)
                .resizable(false)
                .show_separator_line(false)
                .frame(
                    egui::Frame::new()
                        .fill(crate::style::theme::c().split_bg)
                        .inner_margin(egui::Margin {
                            left: 12,
                            right: 8,
                            top: 14,
                            bottom: 8,
                        }),
                )
                .show(ui, |ui| {
                    for name in GENERAL_SECTIONS {
                        let r = ui.add(
                            egui::Label::new(
                                egui::RichText::new(name)
                                    .size(14.0)
                                    .color(crate::style::theme::c().accent),
                            )
                            .sense(Sense::click()),
                        );
                        if r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                            self.settings_ui.scroll_to = Some(name);
                        }
                        ui.add_space(4.0);
                    }
                });
        }
        egui::CentralPanel::no_frame()
            .frame(
                egui::Frame::new()
                    .fill(crate::style::theme::c().split_bg)
                    .inner_margin(egui::Margin {
                        left: 18,
                        right: 10,
                        top: 6,
                        bottom: 6,
                    }),
            )
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("settings-page")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // at most 640 wide, never wider than the window shows (set_max_width
                        // alone can widen it: text then ran off the right edge, unwrapped)
                        ui.set_max_width(ui.available_width().min(640.0));
                        if nothing {
                            ui.add_space(12.0);
                            ui.label(format!(
                                "No settings match \"{}\".",
                                self.settings_ui.search.trim()
                            ));
                            ui.weak("Try fewer words, or the start of a word.");
                            return;
                        }
                        match self.settings_ui.page {
                            Page::General => self.general_page(ui),
                            Page::Accounts => self.accounts_page(ui),
                            Page::Nicknames => self.nicknames_page(ui),
                            Page::Commands => self.commands_page(ui),
                            Page::Highlights => self.highlights_page(ui),
                            Page::Ignores => self.ignores_page(ui),
                            Page::Hotkeys => self.hotkeys_page(ui),
                            Page::Filters => self.filters_page(ui),
                            Page::Moderation => self.moderation_page(ui),
                            Page::Plugins => self.plugins_page(ui),
                            Page::Notifications => self.notifications_page(ui),
                            Page::LiveList => self.live_list_page(ui),
                            Page::Platforms => self.channels_page(ui),
                            Page::ExternalTools => self.streamlink_page(ui),
                            Page::About => self.about_page(ui),
                        }
                        ui.add_space(12.0);
                    });
            });
        done
    }

    /// One page row (Moltorino's SettingsDialogTab): 30 px, icon then name; the selected
    /// one on a lighter surface with the name in the accent colour.
    fn settings_tab(&self, ui: &mut egui::Ui, p: Page) -> bool {
        let sel = self.settings_ui.page == p;
        let (rect, r) = ui.allocate_exact_size(Vec2::new(150.0, 30.0), Sense::click());
        let painter = ui.painter();
        if sel {
            painter.rect_filled(rect, 0.0, crate::style::theme::c().shade(34));
        } else if r.hovered() {
            painter.rect_filled(rect, 0.0, crate::style::theme::c().shade(25));
        }
        let color = if sel {
            crate::style::theme::c().accent
        } else {
            crate::style::theme::c().text
        };
        painter.text(
            egui::Pos2::new(rect.left() + 18.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            p.icon(),
            FontId::proportional(14.0),
            if sel {
                color
            } else {
                crate::style::theme::c().tab_text
            },
        );
        painter.text(
            egui::Pos2::new(rect.left() + 36.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            p.label(),
            FontId::proportional(13.5),
            color,
        );
        r.clicked()
    }

    /// A General section title like chatterino7's ("Interface:" in the accent colour); the
    /// index on the right scrolls to it.
    fn section(&mut self, ui: &mut egui::Ui, name: &'static str) {
        ui.add_space(12.0);
        let bg = ui.painter().add(egui::Shape::Noop);
        let r = ui.label(
            egui::RichText::new(format!("{name}:"))
                .size(22.0)
                .color(crate::style::theme::c().accent),
        );
        mark_found(ui, name, bg, &r);
        if self.settings_ui.scroll_to == Some(name) {
            r.scroll_to_me(Some(egui::Align::TOP));
            self.settings_ui.scroll_to = None;
        }
    }

    /// General > Interface (chatterino7 / Moltorino): theme, accent colour, font, zoom, tab
    /// layout and visibility, tab close button, title bar, always on top, start with Windows.
    /// Changes show at once.
    fn interface_section(&mut self, ui: &mut egui::Ui) {
        let look = &mut self.settings.appearance;
        heading(ui, "Theme", TEXT);
        ui.horizontal(|ui| {
            for t in crate::style::THEMES {
                if ui.selectable_label(look.theme == t, t).clicked() {
                    look.theme = t.to_string();
                }
            }
        });
        ui.weak("Dark, Black, Light and White, with Moltorino's colours.");
        heading(ui, "Accent colour", TEXT);
        ui.horizontal(|ui| {
            let mut c = look.accent.unwrap_or([0x00, 0xae, 0xef]);
            if ui.color_edit_button_srgb(&mut c).changed() {
                look.accent = Some(c);
            }
            ui.label("Selected tabs, the selected settings page and selections");
            if look.accent.is_some() && ui.button("Default").clicked() {
                look.accent = None;
            }
        });
        heading(ui, "Font", TEXT);
        ui.horizontal(|ui| {
            ui.label("Font");
            egui::ComboBox::from_id_salt("look-font")
                .selected_text(look.font.clone())
                .show_ui(ui, |ui| {
                    for (name, _, _) in crate::style::FONTS {
                        ui.selectable_value(&mut look.font, name.to_string(), name);
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.label("Chat text size");
            ui.add(egui::Slider::new(&mut self.settings.chat_font, 10.0..=20.0));
        });
        let look = &mut self.settings.appearance;
        ui.horizontal(|ui| {
            ui.label("Zoom");
            ui.add(
                egui::Slider::new(&mut look.zoom, 0.8..=1.5)
                    .step_by(0.05)
                    .fixed_decimals(2),
            );
            if (look.zoom - 1.0).abs() > f32::EPSILON && ui.button("100%").clicked() {
                look.zoom = 1.0;
            }
        });
        heading(ui, "Tabs", TEXT);
        ui.horizontal(|ui| {
            ui.label("Tabs are at the");
            for pos in ["Top", "Left", "Right", "Bottom"] {
                if ui.selectable_label(look.tab_position == pos, pos).clicked() {
                    look.tab_position = pos.to_string();
                }
            }
        });
        ui.checkbox(
            &mut look.tab_close_buttons,
            "Show a close button (×) on tabs",
        );
        ui.horizontal(|ui| {
            ui.label("Tab visibility");
            if ui
                .selectable_label(!self.settings.tab_live_only, "All tabs")
                .clicked()
            {
                self.settings.tab_live_only = false;
            }
            if ui
                .selectable_label(self.settings.tab_live_only, "Only live tabs")
                .clicked()
            {
                self.settings.tab_live_only = true;
            }
        });
        ui.checkbox(
            &mut self.settings.tabs_locked,
            "Lock tab layout (tabs can't be moved or closed)",
        )
        .on_hover_text("Drag a tab to move it. Right-click a tab to lock just that one.");
        ui.add_enabled(
            self.settings.tabs_locked,
            egui::Checkbox::new(
                &mut self.settings.tabs_hide_lock,
                "Hide tab close buttons (and padlocks) while the layout is locked",
            ),
        );
        ui.checkbox(
            &mut self.settings.tabs_compact,
            "Thinner tab lines and tighter tab spacing",
        );
        let look = &mut self.settings.appearance;
        heading(ui, "Window", TEXT);
        ui.checkbox(&mut look.own_title_bar, "Use Yaperino's own title bar (account, settings and window buttons on top, like Chatterino)");
        ui.checkbox(&mut self.settings.always_on_top, "Always on top");
        if cfg!(windows) {
            ui.checkbox(
                &mut self.settings.hide_to_tray,
                "Close to the tray: closing hides Yaperino by the clock (its icon brings it back, its menu quits)",
            );
        }
        // "Start with Windows" uses the Windows registry
        if cfg!(windows) {
            let installed = *self
                .settings_ui
                .installed
                .get_or_insert_with(crate::install::is_installed);
            let mut start = self.settings.start_with_windows;
            let r = ui.add_enabled(
                installed || start,
                egui::Checkbox::new(&mut start, "Start with Windows"),
            );
            let r = r.on_disabled_hover_text(
                "Install Yaperino first (About > Install), so Windows starts the installed copy",
            );
            if r.changed() {
                match super::features::set_start_with_windows(start) {
                    Ok(()) => {
                        self.settings.start_with_windows = start;
                        self.settings_ui.start_message.clear();
                    }
                    Err(e) => self.settings_ui.start_message = e,
                }
            }
            if !self.settings_ui.start_message.is_empty() {
                ui.weak(&self.settings_ui.start_message);
            }
        }
        ui.add_space(8.0);
        if ui.button("Reset the look").clicked() {
            self.settings.appearance = Default::default();
        }
    }

    // -------------------------------------------------------------- General
    /// chatterino7's General page: Interface, Chat (messages, completion, split header),
    /// Emotes, with the index on the right.
    fn general_page(&mut self, ui: &mut egui::Ui) {
        self.section(ui, "Interface");
        self.interface_section(ui);
        self.section(ui, "Chat");
        self.chat_page(ui);
        self.section(ui, "Translation");
        self.translation_section(ui);
        self.section(ui, "Emotes");
        self.emotes_page(ui);
    }
    /// General > Translation (Moltorino): the language, the service (Google, or DeepL with
    /// your own key) and the auto-translated chats.
    fn translation_section(&mut self, ui: &mut egui::Ui) {
        ui.label("Right-click a message > Translate: its translation appears under it.");
        ui.label("Split menu > Auto-translate this chat: new messages in other languages get translated.");
        heading(ui, "Translate to", TEXT);
        let s = &mut self.settings;
        ui.horizontal(|ui| {
            ui.label("Language");
            egui::ComboBox::from_id_salt("translate-to")
                .selected_text(super::translate::language_name(&s.translate_to))
                .height(320.0)
                .show_ui(ui, |ui| {
                    for (code, name) in super::translate::LANGUAGES {
                        ui.selectable_value(&mut s.translate_to, code.to_string(), name);
                    }
                });
        });
        heading(ui, "Service", TEXT);
        ui.horizontal(|ui| {
            for p in ["Google", "DeepL"] {
                if ui.selectable_label(s.translate_provider == p, p).clicked() {
                    s.translate_provider = p.to_string();
                }
            }
        });
        if s.translate_provider == "DeepL" {
            ui.horizontal(|ui| {
                ui.label("DeepL key");
                ui.add(
                    egui::TextEdit::singleline(&mut s.deepl_key)
                        .password(true)
                        .hint_text("from your DeepL account (free keys end in :fx)")
                        .desired_width(320.0),
                );
            });
            ui.weak("Your own DeepL API key (free DeepL accounts get 500,000 characters a month).");
            ui.weak("The key is never put in settings backups.");
        } else {
            ui.weak("Google's web translator: free, no key needed.");
        }
        ui.weak("Only the message's text is sent, only when translating: to Google, or DeepL with your key.");
        ui.weak("At most one message a second; in busy chats some messages are skipped.");
        if !s.auto_translate.is_empty() {
            ui.label("Auto-translated chats (click one to stop):");
            let mut remove = None;
            ui.horizontal_wrapped(|ui| {
                for (i, c) in s.auto_translate.iter().enumerate() {
                    if ui.small_button(format!("{c}  ✕")).clicked() {
                        remove = Some(i);
                    }
                }
            });
            if let Some(i) = remove {
                s.auto_translate.remove(i);
            }
        }
    }
    // ------------------------------------------------------------- Accounts

    fn accounts_page(&mut self, ui: &mut egui::Ui) {
        let mut action: Option<AccountMode> = None;
        let mut start_device = false;
        let mut add_login: Option<SavedDevice> = None;
        let mut kick_sign_in = false;
        let mut kick_sign_out = false;
        let mut yt_sign_in = false;
        let mut yt_sign_out = false;

        heading(ui, "Twitch", Platform::Twitch.color());
        ui.label(egui::RichText::new(&self.account_status).strong());
        // signed in before the mod tools were added: the new permissions need a new sign-in
        if let Some(missing) = self
            .account
            .as_ref()
            .map(|a| a.missing_scopes())
            .filter(|m| !m.is_empty())
        {
            ui.colored_label(
                Color32::from_rgb(230, 160, 80),
                format!("Sign in again (Add an account > Sign in with Twitch, same account) to allow the new permissions for the mod tools: {}", missing.join(", ")),
            );
        }
        ui.label("Signed in, the live list also shows every channel you follow that is live, and you can send messages.");
        // the account list (like Chatterino's account switcher)
        let mut use_account: Option<String> = None;
        let mut remove_account: Option<String> = None;
        egui::Frame::new()
            .fill(Color32::from_rgb(38, 38, 38))
            .inner_margin(6.0)
            .corner_radius(4.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                if ui
                    .selectable_label(
                        self.settings.account == AccountMode::Off,
                        "Anonymous (read only)",
                    )
                    .clicked()
                {
                    action = Some(AccountMode::Off);
                }
                for a in &self.settings.twitch_accounts {
                    let current = self.settings.account != AccountMode::Off
                        && a.username == self.settings.current_twitch;
                    ui.horizontal(|ui| {
                        if ui.selectable_label(current, &a.username).clicked() {
                            use_account = Some(a.username.clone());
                        }
                        ui.weak(if a.refresh.is_empty() {
                            "pasted login"
                        } else {
                            "device login (renews itself)"
                        });
                        if ui.small_button("Remove").clicked() {
                            remove_account = Some(a.username.clone());
                        }
                    });
                }
                if self.settings.twitch_accounts.is_empty() {
                    ui.weak("No accounts yet: add one below.");
                }
            });

        ui.add_space(6.0);
        ui.label(egui::RichText::new("Add an account").strong());
        // a build without a built-in Twitch app (a published copy) needs a Client ID first
        let no_app = account::DEFAULT_CLIENT_ID.is_empty()
            && self.settings.twitch_client_id.trim().is_empty();
        if no_app {
            ui.label("This copy of Yaperino has no Twitch app built in. Register one (Public client) at dev.twitch.tv/console and enter its Client ID:");
            ui.horizontal(|ui| {
                ui.label("Client ID");
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings.twitch_client_id)
                        .desired_width(260.0),
                );
            });
        }
        // 1. device login, as Moltorino
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    self.acc_dialog.waiting.is_none() && !no_app,
                    egui::Button::new("Sign in with device login"),
                )
                .clicked()
            {
                start_device = true;
            }
            ui.label("Opens Twitch Activate and copies a code for you.");
        });
        if let Some((code, uri)) = &self.acc_dialog.waiting {
            ui.horizontal(|ui| {
                ui.label("Code:");
                ui.label(egui::RichText::new(code).monospace().size(18.0).strong());
                if ui.button("Copy code").clicked() {
                    ui.ctx().copy_text(code.clone());
                }
                if ui.button("Open Twitch Activate").clicked() {
                    browser::open_url(uri);
                }
            });
        }
        // 2. paste login info, as Chatterino
        ui.add_space(4.0);
        ui.label("Or paste login info (oauth_token=...;client_id=...;username=...;user_id=...):");
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.acc_dialog.paste)
                    .password(true)
                    .desired_width(340.0),
            );
            if ui
                .add_enabled(
                    !self.acc_dialog.paste.trim().is_empty(),
                    egui::Button::new("Add"),
                )
                .clicked()
            {
                match account::parse_login_info(&self.acc_dialog.paste) {
                    Some(s) => add_login = Some(s),
                    None => {
                        self.acc_dialog.message =
                            "That doesn't look like login info (it needs oauth_token and client_id)"
                                .into()
                    }
                }
                self.acc_dialog.paste.clear();
            }
        });
        // 3. advanced, as Chatterino
        ui.collapsing("Advanced", |ui| {
            ui.label(
                "1. Fill in your username  2. user ID  3. client ID  4. OAuth token  5. Add user",
            );
            egui::Grid::new("adv-login").num_columns(2).show(ui, |ui| {
                for (i, label) in ["Username", "User ID", "Client ID", "OAuth token"]
                    .iter()
                    .enumerate()
                {
                    ui.label(*label);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.acc_dialog.adv[i])
                            .password(i == 3)
                            .desired_width(260.0),
                    );
                    ui.end_row();
                }
            });
            ui.horizontal(|ui| {
                ui.label("Device login Client ID:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings.twitch_client_id)
                        .hint_text(account::DEFAULT_CLIENT_ID)
                        .desired_width(260.0),
                );
            });
            ui.weak("Leave empty to use this app's own Twitch app.");
            ui.horizontal(|ui| {
                let ready = self.acc_dialog.adv.iter().all(|f| !f.trim().is_empty());
                if ui
                    .add_enabled(ready, egui::Button::new("Add user"))
                    .clicked()
                {
                    let f = &self.acc_dialog.adv;
                    add_login = Some(SavedDevice {
                        username: f[0].trim().to_lowercase(),
                        user_id: f[1].trim().to_string(),
                        client_id: f[2].trim().to_string(),
                        token: f[3].trim().trim_start_matches("oauth:").to_string(),
                        refresh: String::new(),
                    });
                }
                if ui.button("Clear fields").clicked() {
                    self.acc_dialog.adv = Default::default();
                }
            });
        });
        if !self.acc_dialog.message.is_empty() {
            ui.label(&self.acc_dialog.message);
        }
        if let Some(name) = use_account {
            self.settings.current_twitch = name;
            action = Some(AccountMode::Device);
        }
        if let Some(name) = remove_account {
            self.settings.twitch_accounts.retain(|a| a.username != name);
            if self.settings.current_twitch == name {
                self.settings.current_twitch.clear();
            }
            action = Some(self.settings.account);
        }
        ui.separator();
        heading(ui, "Kick", Platform::Kick.color());
        ui.weak("Kick chat and the live check work without an account. To send messages, sign in on kick.com in the sign-in window (no developer app needed).");
        let mut kick_web = false;
        ui.horizontal(|ui| {
            if self.settings.kick.signed_in() {
                let name = if self.settings.kick.username.is_empty() {
                    "your Kick account"
                } else {
                    &self.settings.kick.username
                };
                ui.label(format!("Signed in as {name}"));
                if ui.button("Sign out").clicked() {
                    kick_sign_out = true;
                }
            } else if ui
                .add_enabled(
                    !self.settings_ui.kick_busy,
                    egui::Button::new("Sign in with Kick"),
                )
                .clicked()
            {
                kick_web = true;
            }
            if self.settings_ui.kick_busy {
                ui.spinner();
                ui.label("Log in on kick.com in the window that opened...");
            }
        });
        if !self.settings_ui.kick_message.is_empty() {
            ui.label(&self.settings_ui.kick_message);
        }
        ui.collapsing("Advanced: sign in with your own Kick app instead", |ui| {
            ui.label("1. Open kick.com/settings/developer and create an app.");
            ui.label(format!("2. Add this redirect URL: {}", crate::kick_auth::REDIRECT));
            ui.label("3. Allow the scopes: user:read, channel:read, channel:write, chat:write (and moderation if you moderate).");
            ui.label("4. Copy its Client ID and Client Secret here and press Sign in with your app.");
            egui::Grid::new("kick-grid").num_columns(2).show(ui, |ui| {
                ui.label("Client ID");
                ui.add(egui::TextEdit::singleline(&mut self.settings_ui.kick_client_id).desired_width(320.0));
                ui.end_row();
                ui.label("Client secret");
                ui.add(egui::TextEdit::singleline(&mut self.settings_ui.kick_client_secret).password(true).desired_width(320.0));
                ui.end_row();
            });
            let ready = !self.settings_ui.kick_busy
                && !self.settings_ui.kick_client_id.trim().is_empty()
                && !self.settings_ui.kick_client_secret.trim().is_empty();
            if ui.add_enabled(ready, egui::Button::new("Sign in with your app")).clicked() {
                kick_sign_in = true;
            }
        });

        ui.separator();
        heading(ui, "YouTube", Platform::YouTube.color());
        ui.weak("The live check and reading chat work without an account. To send messages, sign in with Google in the sign-in window (no developer app needed). Your Google account needs a YouTube channel to chat.");
        let mut yt_web = false;
        ui.horizontal(|ui| {
            if self.settings.youtube.signed_in() {
                let name = if self.settings.youtube.channel_name.is_empty() {
                    "your channel"
                } else {
                    &self.settings.youtube.channel_name
                };
                ui.label(format!("Signed in as {name}"));
                if ui.button("Sign out").clicked() {
                    yt_sign_out = true;
                }
            } else if ui
                .add_enabled(
                    !self.settings_ui.yt_busy,
                    egui::Button::new("Sign in with YouTube"),
                )
                .clicked()
            {
                yt_web = true;
            }
            if self.settings_ui.yt_busy {
                ui.spinner();
                ui.label("Log in with Google in the window that opened...");
            }
        });
        if !self.settings_ui.yt_message.is_empty() {
            ui.label(&self.settings_ui.yt_message);
        }
        ui.collapsing("Advanced: sign in with your own Google app instead", |ui| {
            ui.label("If Google refuses the sign-in window (\"This browser or app may not be secure\"), use a Google app once:");
            ui.label("1. On console.cloud.google.com create a project and turn on \"YouTube Data API v3\".");
            ui.label("2. OAuth consent screen: External; add your own Google account as a test user.");
            ui.label("3. Credentials > Create OAuth client ID > Desktop app. Copy its Client ID and Client secret here.");
            ui.label("While your Google app is in Testing, Google ends the sign-in after about a week: sign in again then, or publish the app.");
            egui::Grid::new("yt-grid").num_columns(2).show(ui, |ui| {
                ui.label("Client ID");
                ui.add(egui::TextEdit::singleline(&mut self.settings_ui.yt_client_id).desired_width(320.0));
                ui.end_row();
                ui.label("Client secret");
                ui.add(egui::TextEdit::singleline(&mut self.settings_ui.yt_client_secret).password(true).desired_width(320.0));
                ui.end_row();
            });
            let ready = !self.settings_ui.yt_busy
                && !self.settings_ui.yt_client_id.trim().is_empty()
                && !self.settings_ui.yt_client_secret.trim().is_empty();
            if ui.add_enabled(ready, egui::Button::new("Sign in with your app")).clicked() {
                yt_sign_in = true;
            }
        });

        ui.separator();
        heading(ui, "TikTok", Platform::TikTok.color());
        ui.weak("The live check and reading chat work without an account. Sending to TikTok needs TikTok's anti-bot request signatures, which this app does not rebuild, so TikTok is read only here.");

        // ---- apply
        if kick_sign_out {
            self.settings.kick = Default::default();
            crate::weblogin::forget("kick");
            self.settings_ui.kick_message = "Signed out of Kick".into();
            self.load_kick_account();
        }
        if kick_web {
            self.settings_ui.kick_busy = true;
            self.settings_ui.kick_message.clear();
            let (tx, ctx) = (self.side_tx.clone(), self.ctx.clone());
            std::thread::spawn(move || {
                let r = crate::weblogin::sign_in("kick").map(|session| {
                    // who is signed in (only reads); the sign-in counts even if Kick doesn't say
                    let (username, user_id) =
                        crate::kick_auth::web_whoami(&session).unwrap_or_default();
                    crate::kick_auth::KickSaved {
                        username,
                        user_id,
                        session,
                        ..Default::default()
                    }
                });
                let _ = tx.send(SideMsg::KickSignedIn(r));
                ctx.request_repaint();
            });
        }
        if yt_web {
            self.settings_ui.yt_busy = true;
            self.settings_ui.yt_message.clear();
            let (tx, ctx) = (self.side_tx.clone(), self.ctx.clone());
            std::thread::spawn(move || {
                let r = crate::weblogin::sign_in("youtube").map(|session| {
                    let channel_name =
                        crate::youtube_auth::web_channel_name(&session).unwrap_or_default();
                    crate::youtube_auth::YouTubeSaved {
                        channel_name,
                        session,
                        ..Default::default()
                    }
                });
                let _ = tx.send(SideMsg::YouTubeSignedIn(r));
                ctx.request_repaint();
            });
        }
        if kick_sign_in {
            self.settings_ui.kick_busy = true;
            self.settings_ui.kick_message.clear();
            let (id, secret) = (
                self.settings_ui.kick_client_id.trim().to_string(),
                self.settings_ui.kick_client_secret.trim().to_string(),
            );
            let (tx, ctx) = (self.side_tx.clone(), self.ctx.clone());
            std::thread::spawn(move || {
                let r = crate::kick_auth::sign_in(&id, &secret, |url| browser::open_url(url));
                let _ = tx.send(SideMsg::KickSignedIn(r));
                ctx.request_repaint();
            });
        }
        if let Some(mode) = action {
            self.settings.account = mode;
            self.load_account();
        }
        if start_device {
            let client_id = self.twitch_client_id();
            self.acc_dialog.message = "Asking Twitch for a code...".into();
            let (tx, ctx) = (self.acc_tx.clone(), self.ctx.clone());
            std::thread::spawn(move || {
                let msg = match account::device_start(&client_id) {
                    Ok((device_code, user_code, uri, interval)) => AccountMsg::DeviceCode {
                        device_code,
                        user_code,
                        uri,
                        interval,
                    },
                    Err(e) => AccountMsg::DeviceDone(Err(e)),
                };
                let _ = tx.send(msg);
                ctx.request_repaint();
            });
        }
        if let Some(s) = add_login {
            self.acc_dialog.adv = Default::default();
            self.acc_dialog.message = "Checking the login with Twitch...".into();
            let (tx, ctx) = (self.acc_tx.clone(), self.ctx.clone());
            std::thread::spawn(move || {
                let _ = tx.send(AccountMsg::DeviceDone(account::complete(s)));
                ctx.request_repaint();
            });
        }
        if yt_sign_out {
            self.settings.youtube = Default::default();
            crate::weblogin::forget("youtube");
            self.settings_ui.yt_message = "Signed out of YouTube".into();
        }
        if yt_sign_in {
            self.settings_ui.yt_busy = true;
            self.settings_ui.yt_message.clear();
            let (id, secret) = (
                self.settings_ui.yt_client_id.trim().to_string(),
                self.settings_ui.yt_client_secret.trim().to_string(),
            );
            let (tx, ctx) = (self.side_tx.clone(), self.ctx.clone());
            std::thread::spawn(move || {
                let r = crate::youtube_auth::sign_in(&id, &secret, |url| browser::open_url(url));
                let _ = tx.send(SideMsg::YouTubeSignedIn(r));
                ctx.request_repaint();
            });
        }
    }

    // -------------------------------------------------------------- General
    /// Where the live list comes from: the Twitch sign-in's follows, never a file.
    fn list_source(&mut self, ui: &mut egui::Ui) {
        heading(ui, "Where the live list comes from", TEXT);
        match self.account.as_ref().filter(|a| a.can_follow()) {
            Some(a) => ui.label(format!(
                "Every channel {} follows on Twitch ({} channels), checked every {} s, plus the channels below and every channel you join in a tab.",
                a.login,
                self.settings.follows.len(),
                self.settings.refresh_secs
            )),
            None => ui.label("Sign in with Twitch (Accounts) and the list shows every channel you follow, live. Without it only the channels below and the ones you join are checked."),
        };
    }

    fn live_list_page(&mut self, ui: &mut egui::Ui) {
        self.list_source(ui);
        heading(ui, "Live list", TEXT);
        ui.horizontal(|ui| {
            ui.label("Check for live streams every");
            ui.add(
                egui::DragValue::new(&mut self.settings.refresh_secs)
                    .range(30..=1800)
                    .suffix(" s"),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Sort by");
            let mut sort = self.settings.sort;
            egui::ComboBox::from_id_salt("settings-sort")
                .selected_text(sort.label())
                .show_ui(ui, |ui| {
                    for m in SortMode::ALL {
                        ui.selectable_value(&mut sort, m, m.label());
                    }
                });
            if sort != self.settings.sort {
                self.set_sort(sort);
            }
        });
        let mut ff = self.settings.fav_first;
        if ui.checkbox(&mut ff, "Favorites first").changed() {
            self.settings.fav_first = ff;
            self.resort();
        }
        ui.checkbox(&mut self.settings.favs_only, "Show favorites only");
        ui.checkbox(
            &mut self.settings.group_by_platform,
            "One section per platform (Twitch, Kick, YouTube, TikTok)",
        );
        ui.checkbox(
            &mut self.settings.match_names,
            "Find followed and joined channels on Kick, YouTube and TikTok by the same name, and list them there when live",
        );
        let found = self
            .settings
            .matched
            .values()
            .filter(|l| !l.is_empty())
            .count();
        ui.weak(format!(
            "Looked up {} channels by name, {found} found on another platform{}. Channels joined in a tab are listed whether you follow them or not.",
            self.settings.matched.len(),
            if self.match_queue.is_empty() { String::new() } else { format!(", {} still to look up", self.match_queue.len()) }
        ));
        ui.add_space(6.0);
        ui.label(format!(
            "{} favourite channels, {} favourite games",
            self.settings.fav_channels.len(),
            self.settings.fav_games.len()
        ));
        if ui.button("Clear favourites").clicked() {
            self.settings.fav_channels.clear();
            self.settings.fav_games.clear();
            self.resort();
        }
    }

    // ------------------------------------------------------------- Channels

    fn channels_page(&mut self, ui: &mut egui::Ui) {
        heading(ui, "Added channels", TEXT);
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.settings_ui.add_channel)
                    .hint_text("twitch name")
                    .desired_width(200.0),
            );
            if ui.button("Add").clicked() {
                if let Some(n) = fetch::valid_login(&self.settings_ui.add_channel) {
                    if !self.settings.extra.contains(&n) {
                        self.settings.extra.insert(0, n);
                    }
                    self.settings_ui.add_channel.clear();
                }
            }
        });
        let mut remove: Option<usize> = None;
        ui.horizontal_wrapped(|ui| {
            for (i, c) in self.settings.extra.iter().enumerate() {
                if ui
                    .button(format!("{c}  ✕"))
                    .on_hover_text("Remove")
                    .clicked()
                {
                    remove = Some(i);
                }
            }
        });
        if let Some(i) = remove {
            self.settings.extra.remove(i);
        }

        heading(ui, "Streamers on other platforms", TEXT);
        ui.label("Opening a chat tab joins every platform listed here. Found automatically from the Twitch and Kick profiles, or by the same name (marked).");
        if !self.settings.links.is_empty() {
            ui.weak("One row per streamer. White: the platform the chat tab was opened on.");
        }
        let mut keys: Vec<String> = self.settings.links.keys().cloned().collect();
        keys.sort_by_key(|k| super::tab_primary(k).1.to_lowercase());
        let mut edit: Option<String> = None;
        let mut forget: Option<String> = None;
        egui::Grid::new("links-table")
            .num_columns(6)
            .striped(true)
            .spacing([14.0, 4.0])
            .show(ui, |ui| {
                if !keys.is_empty() {
                    for h in ["Twitch", "Kick", "YouTube", "TikTok", "Found from", ""] {
                        ui.label(egui::RichText::new(h).strong());
                    }
                    ui.end_row();
                }
                for k in &keys {
                    let l = &self.settings.links[k];
                    let (own, own_name) = super::tab_primary(k);
                    for p in Platform::ALL {
                        // the tab's own channel, also when the links don't repeat it
                        let v = if p == own && l.get(p).is_empty() {
                            own_name.as_str()
                        } else {
                            l.get(p)
                        };
                        if v.is_empty() {
                            ui.label(egui::RichText::new("-").color(Color32::from_gray(0x66)));
                        } else if p == own {
                            ui.label(
                                egui::RichText::new(v).color(crate::style::theme::c().strong_text),
                            );
                        } else {
                            ui.label(
                                egui::RichText::new(v).color(crate::style::theme::c().tab_text),
                            );
                        }
                    }
                    ui.label(if l.found_from.is_empty() {
                        "by hand"
                    } else {
                        &l.found_from
                    });
                    ui.horizontal(|ui| {
                        if ui.small_button("Edit").clicked() {
                            edit = Some(k.clone());
                        }
                        if ui.small_button("Forget").clicked() {
                            forget = Some(k.clone());
                        }
                    });
                    ui.end_row();
                }
            });
        if keys.is_empty() {
            ui.weak("None yet: open a chat tab, or right-click a stream > Platforms..");
        }
        if let Some(k) = edit {
            let l = self.settings.links.get(&k).cloned().unwrap_or_default();
            self.links_edit = Some((k, l));
        }
        if let Some(k) = forget {
            self.settings.links.remove(&k);
        }
    }

    // ----------------------------------------------------------------- Chat

    fn chat_page(&mut self, ui: &mut egui::Ui) {
        heading(ui, "Messages", TEXT);
        ui.horizontal(|ui| {
            ui.label("Text size");
            ui.add(egui::Slider::new(&mut self.settings.chat_font, 10.0..=20.0));
        });
        ui.checkbox(
            &mut self.settings.timestamps,
            "Show the time before each message",
        );
        ui.checkbox(
            &mut self.settings.restore_tabs,
            "Open my chat tabs again when the app starts",
        );
        ui.checkbox(
            &mut self.settings.alternate_bg,
            "Alternate the background of every other message",
        );
        ui.checkbox(
            &mut self.settings.separators,
            "Draw a line between messages",
        );
        ui.checkbox(
            &mut self.settings.highlight_mentions,
            "Highlight messages that mention my name",
        );
        ui.checkbox(&mut self.settings.show_badges, "Show badges");
        ui.checkbox(
            &mut self.settings.platform_badges,
            "Show a platform badge before each message (Twitch, Kick, YouTube, TikTok)",
        );
        ui.checkbox(
            &mut self.settings.source_background,
            "Show the message source as a platform-coloured background",
        );
        ui.checkbox(
            &mut self.settings.link_previews,
            "Link previews: hovering a link shows its title and picture (Yaperino asks the link's website)",
        );
        ui.checkbox(
            &mut self.settings.collapse_repeats,
            "Collapse repeated messages (the same message again from the same user shows ×N)",
        );
        ui.checkbox(
            &mut self.settings.load_history,
            "Load the recent chat history when a Twitch chat opens (from recent-messages.robotty.de, like Chatterino)",
        );
        ui.checkbox(&mut self.settings.bold_names, "Bold user names");
        ui.checkbox(
            &mut self.settings.user_colors,
            "Colour user names with their own colours",
        );
        ui.horizontal(|ui| {
            ui.label("Messages kept per chat");
            ui.add(
                egui::DragValue::new(&mut self.settings.message_limit)
                    .range(50..=5000)
                    .speed(10),
            );
        });
        heading(ui, "Completion", TEXT);
        ui.checkbox(
            &mut self.settings.completion_popup,
            "Suggest emotes while typing :name and chatters while typing @name",
        );
        ui.checkbox(&mut self.settings.tab_completion, "Tab completes emotes and user names (press again for the next match, Shift+Tab goes back)");
        ui.checkbox(
            &mut self.settings.emote_carousel,
            "Emote carousel: show emote suggestions as a row of pictures that slides with each Tab press",
        );
        heading(ui, "Spellcheck", TEXT);
        ui.checkbox(
            &mut self.settings.spellcheck,
            "Check spelling in the message box: misspelled words get a red line, right-click one for suggestions",
        );
        ui.weak("Emotes, names, @mentions, links and common chat words are left alone.");
        // the languages, as chips: a word is right when any of them knows it
        ui.add_space(4.0);
        ui.label("Languages");
        let p = crate::style::theme::c();
        let name_of = |code: &str| {
            crate::spell::LANGUAGES
                .iter()
                .find(|(c, _)| *c == code)
                .map_or(code.to_string(), |(_, n)| n.to_string())
        };
        let mut remove: Option<String> = None;
        let mut add: Option<String> = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            for code in self.settings.spell_languages.clone() {
                let state = crate::spell::status(&code);
                let (mark, tip) = match &state {
                    Some(Ok(false)) => ("⟳", "Downloading...".to_string()),
                    Some(Err(e)) => ("⚠", format!("Couldn't load it: {e}")),
                    Some(Ok(true)) => ("✔", "Ready".to_string()),
                    None => ("", "Loaded once you type".to_string()),
                };
                let fill = if matches!(state, Some(Err(_))) {
                    Color32::from_rgb(0xe8, 0x48, 0x48).gamma_multiply(0.25)
                } else {
                    p.accent.gamma_multiply(0.22)
                };
                if lang_chip(ui, &name_of(&code), mark, &tip, fill) {
                    remove = Some(code.clone());
                }
            }
            // + Add language: a searchable list of the others
            let search = &mut self.settings_ui.lang_search;
            let picked = &self.settings.spell_languages;
            ui.menu_button(
                egui::RichText::new("+ Add language").color(p.accent),
                |ui| {
                    ui.set_min_width(220.0);
                    ui.set_max_width(240.0);
                    ui.add(
                        egui::TextEdit::singleline(search)
                            .hint_text("Search languages...")
                            .desired_width(220.0),
                    )
                    .request_focus();
                    let q = search.trim().to_lowercase();
                    egui::ScrollArea::vertical()
                        .max_height(260.0)
                        .show(ui, |ui| {
                            for (code, name) in crate::spell::LANGUAGES {
                                if picked.iter().any(|l| l == code)
                                    || !(q.is_empty()
                                        || name.to_lowercase().contains(&q)
                                        || code.to_lowercase().contains(&q))
                                {
                                    continue;
                                }
                                if ui.button(*name).clicked() {
                                    add = Some(code.to_string());
                                    ui.close();
                                }
                            }
                        });
                },
            );
        });
        if let Some(code) = remove {
            self.settings.spell_languages.retain(|l| *l != code);
        }
        if let Some(code) = add {
            self.settings.spell_languages.push(code);
            self.settings_ui.lang_search.clear();
        }
        ui.weak("English is built in; other languages are downloaded once (from github.com/wooorm/dictionaries, each under its own open licence) and kept in Yaperino's Dictionaries folder.");
        if !self.settings.spell_words.is_empty() {
            ui.label("Your own words (click one to remove it):");
            let mut remove = None;
            ui.horizontal_wrapped(|ui| {
                for (i, w) in self.settings.spell_words.iter().enumerate() {
                    if ui.small_button(format!("{w}  ✕")).clicked() {
                        remove = Some(i);
                    }
                }
            });
            if let Some(i) = remove {
                self.settings.spell_words.remove(i);
            }
        }
        heading(ui, "Message box", TEXT);
        ui.checkbox(&mut self.settings.input_counter, "Show the message length");
        heading(ui, "Split header", TEXT);
        ui.label("Like Chatterino: what the header of a live channel's split shows (hover it for a stream preview).");
        ui.checkbox(&mut self.settings.header_viewers, "Viewer count");
        ui.checkbox(&mut self.settings.header_uptime, "Uptime");
        ui.checkbox(&mut self.settings.header_title, "Stream title");
        ui.checkbox(&mut self.settings.header_game, "Game / category");
        ui.checkbox(
            &mut self.settings.tab_live_dot,
            "Show a red dot on tabs whose channel is live",
        );
        heading(ui, "Windows and shortcuts", TEXT);
        ui.checkbox(
            &mut self.settings.live_on_start,
            "Open the Live Streams window when the app starts",
        );
        ui.label("Alt+L: Live Streams window   Ctrl+T: new split   Ctrl+Shift+T: new tab   Ctrl+W: close split   Ctrl+Tab: next tab   Ctrl+P: settings");
        ui.add_space(6.0);
        ui.label(
            "Emotes: Twitch, 7TV, BTTV and FFZ (chatterino7's order), Kick and YouTube emotes.",
        );
        ui.label("Platform tags before names: K = Kick, YT = YouTube, TT = TikTok.");
    }

    // --------------------------------------------------------------- Emotes

    fn emotes_page(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(
            &mut self.settings.show_emote_images,
            "Show emotes as pictures (off: show their names)",
        );
        ui.horizontal(|ui| {
            ui.label("Emote size");
            ui.add(
                egui::Slider::new(&mut self.settings.emote_scale, 0.5..=2.5)
                    .step_by(0.05)
                    .suffix("x"),
            );
        });
        ui.checkbox(
            &mut self.settings.emote_tooltips,
            "Show the emote name and source when hovering",
        );
        heading(ui, "Emote sources", TEXT);
        ui.label("Third-party emotes are found in chatterino7's order: the channel's FFZ, BTTV, 7TV, then the global ones.");
        let mut changed = false;
        for (i, name) in ["FrankerFaceZ (FFZ)", "BetterTTV (BTTV)", "7TV"]
            .iter()
            .enumerate()
        {
            let mut on = !self.settings.emotes_off[i];
            if ui.checkbox(&mut on, *name).changed() {
                self.settings.emotes_off[i] = !on;
                changed = true;
            }
        }
        if changed {
            self.apply_emote_switches();
        }
        heading(ui, "Emote menu", TEXT);
        ui.horizontal(|ui| {
            ui.label("Remember the last");
            ui.add(egui::DragValue::new(&mut self.settings.recent_limit).range(0..=100));
            ui.label("used emotes");
        });
        if ui.button("Clear recent emotes").clicked() {
            self.settings.recent_emotes.clear();
        }
        ui.checkbox(
            &mut self.settings.close_menu_on_pick,
            "Close the emote menu after picking an emote",
        );
        ui.add_space(6.0);
        ui.weak(format!(
            "Loaded: {} Twitch emotes for your account, {} global FFZ/BTTV/7TV emotes.",
            self.emotes.user_twitch.len(),
            self.emotes.global.len()
        ));
    }

    // ----------------------------------------------------------- Streamlink

    fn streamlink_page(&mut self, ui: &mut egui::Ui) {
        heading(ui, "Streamlink", TEXT);
        ui.label("\"Open in Streamlink\" (right-click a stream or a chat tab) plays the stream in your video player.");
        let found = crate::streamlink::find(&self.settings.streamlink);
        match &found {
            Some(p) => ui.label(format!("Using {}", p.display())),
            None => ui.colored_label(
                Color32::from_rgb(230, 120, 80),
                "Not found: install it from streamlink.github.io, or set its path below.",
            ),
        };
        egui::Grid::new("sl-grid").num_columns(2).show(ui, |ui| {
            ui.label("Path (optional)");
            ui.add(
                egui::TextEdit::singleline(&mut self.settings.streamlink.path)
                    .hint_text("found automatically")
                    .desired_width(380.0),
            );
            ui.end_row();
            ui.label("Quality");
            ui.add(
                egui::TextEdit::singleline(&mut self.settings.streamlink.quality)
                    .hint_text("best")
                    .desired_width(120.0),
            );
            ui.end_row();
            ui.label("Extra options");
            ui.add(
                egui::TextEdit::singleline(&mut self.settings.streamlink.extra_args)
                    .hint_text("e.g. --twitch-low-latency")
                    .desired_width(380.0),
            );
            ui.end_row();
        });
        ui.horizontal(|ui| {
            if ui.button("Check version").clicked() {
                self.settings_ui.streamlink_version = Some(
                    crate::streamlink::version(&self.settings.streamlink).unwrap_or_else(|e| e),
                );
            }
            if let Some(v) = &self.settings_ui.streamlink_version {
                ui.label(v);
            }
        });
        heading(ui, "Custom stream player", TEXT);
        ui.label("You can open streams directly in any video player that has its own URI scheme (e.g. PotPlayer with an extension, IINA on macOS).");
        ui.label("With this set, a split's menu gets \"Open in custom player\", which opens the scheme followed by the stream's address.");
        ui.horizontal(|ui| {
            ui.label("Custom stream player URI scheme:");
            ui.add(
                egui::TextEdit::singleline(&mut self.settings.custom_player)
                    .hint_text("custom-player-scheme://")
                    .desired_width(260.0),
            );
        });
        heading(ui, "Image Uploader", TEXT);
        ui.label("Paste an image into a message box to upload it and put its link in the message. You can set a custom host, like imgur.com or s-ul.eu; Yaperino asks before each upload.");
        ui.checkbox(&mut self.settings.uploader_enabled, "Enable image uploader");
        egui::Grid::new("uploader")
            .num_columns(2)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                for (label, value, hint) in [
                    (
                        "Request URL",
                        &mut self.settings.uploader_url,
                        "https://kappa.lol/api/upload",
                    ),
                    ("Form field", &mut self.settings.uploader_field, "file"),
                    (
                        "Extra headers",
                        &mut self.settings.uploader_headers,
                        "Name: value; Name2: value2",
                    ),
                    ("Image link", &mut self.settings.uploader_link, "{link}"),
                    (
                        "Deletion link",
                        &mut self.settings.uploader_delete,
                        "{delete}",
                    ),
                ] {
                    ui.label(label);
                    ui.add(
                        egui::TextEdit::singleline(value)
                            .hint_text(hint)
                            .desired_width(320.0),
                    );
                    ui.end_row();
                }
            });
        ui.weak("Image link and Deletion link take values from the host's JSON answer, e.g. {link} or {data.link}; empty Image link = the whole answer.");
        if ui.button("Reset to kappa.lol").clicked() {
            self.settings.uploader_url = "https://kappa.lol/api/upload".into();
            self.settings.uploader_field = "file".into();
            self.settings.uploader_headers.clear();
            self.settings.uploader_link = "{link}".into();
            self.settings.uploader_delete = "{delete}".into();
        }
    }
}

/// A simple two-column list editor: rows of (a, b) with remove buttons and an add row.
fn pair_list(
    ui: &mut egui::Ui,
    id: &str,
    heads: (&str, &str),
    rows: &mut Vec<(String, String)>,
    new_a: &mut String,
    new_b: &mut String,
) {
    let mut remove: Option<usize> = None;
    egui::Grid::new(id)
        .num_columns(3)
        .striped(true)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            if rows.is_empty() {
                return;
            }
            ui.weak(heads.0);
            ui.weak(heads.1);
            ui.label("");
            ui.end_row();
            for (i, (a, b)) in rows.iter_mut().enumerate() {
                ui.add(egui::TextEdit::singleline(a).desired_width(170.0));
                ui.add(egui::TextEdit::singleline(b).desired_width(260.0));
                if ui.small_button("Remove").clicked() {
                    remove = Some(i);
                }
                ui.end_row();
            }
        });
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(new_a)
                .hint_text(heads.0)
                .desired_width(170.0),
        );
        ui.add(
            egui::TextEdit::singleline(new_b)
                .hint_text(heads.1)
                .desired_width(260.0),
        );
        if ui
            .add_enabled(!new_a.trim().is_empty(), egui::Button::new("Add"))
            .clicked()
        {
            rows.push((new_a.trim().to_string(), new_b.trim().to_string()));
            new_a.clear();
            new_b.clear();
        }
    });
    if let Some(i) = remove {
        rows.remove(i);
    }
}

/// A one-column list editor.
fn word_list(ui: &mut egui::Ui, id: &str, head: &str, rows: &mut Vec<String>, new_a: &mut String) {
    let mut remove: Option<usize> = None;
    egui::Grid::new(id)
        .num_columns(2)
        .striped(true)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            for (i, a) in rows.iter_mut().enumerate() {
                ui.add(egui::TextEdit::singleline(a).desired_width(300.0));
                if ui.small_button("Remove").clicked() {
                    remove = Some(i);
                }
                ui.end_row();
            }
        });
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(new_a)
                .hint_text(head)
                .desired_width(300.0),
        );
        if ui
            .add_enabled(!new_a.trim().is_empty(), egui::Button::new("Add"))
            .clicked()
        {
            rows.push(new_a.trim().to_string());
            new_a.clear();
        }
    });
    if let Some(i) = remove {
        rows.remove(i);
    }
}

/// Chatterino's page tabs (Messages | Users | ...).
fn sub_tabs(ui: &mut egui::Ui, names: &[&str], sel: &mut usize) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (i, n) in names.iter().enumerate() {
            if crate::style::flat_tab(ui, n, *sel == i, crate::style::theme::c().accent).clicked() {
                *sel = i;
            }
        }
    });
    ui.separator();
}

/// Badges Chatterino can highlight: (badge set, shown name).
const BADGES: [(&str, &str); 9] = [
    ("broadcaster", "Broadcaster"),
    ("moderator", "Moderator"),
    ("vip", "VIP"),
    ("subscriber", "Subscriber"),
    ("founder", "Founder"),
    ("staff", "Staff"),
    ("partner", "Verified"),
    ("turbo", "Turbo"),
    ("premium", "Prime"),
];

#[derive(Clone, Copy, PartialEq)]
enum RuleKind {
    Message,
    User,
    Badge,
}

/// Chatterino's highlight table: Pattern, Flash taskbar, Enable regex, Case-sensitive,
/// Play sound, Color (and on / remove / move).
fn highlight_table(
    ui: &mut egui::Ui,
    id: &str,
    kind: RuleKind,
    rows: &mut Vec<HighlightRule>,
    new_a: &mut String,
) {
    let mut remove: Option<usize> = None;
    let mut swap: Option<(usize, usize)> = None;
    let n = rows.len();
    egui::Grid::new(id)
        .num_columns(9)
        .striped(true)
        .spacing([10.0, 4.0])
        .show(ui, |ui| {
            if rows.is_empty() {
                return;
            }
            let head = match kind {
                RuleKind::Message => "Pattern",
                RuleKind::User => "Username",
                RuleKind::Badge => "Badge",
            };
            for h in [
                "On",
                head,
                "Flash taskbar",
                "Enable regex",
                "Case-sensitive",
                "Play sound",
                "Color",
                "",
                "",
            ] {
                ui.weak(h);
            }
            ui.end_row();
            for (i, r) in rows.iter_mut().enumerate() {
                ui.checkbox(&mut r.enabled, "");
                if kind == RuleKind::Badge {
                    let shown = BADGES
                        .iter()
                        .find(|(k, _)| *k == r.pattern)
                        .map(|(_, v)| *v)
                        .unwrap_or(r.pattern.as_str())
                        .to_string();
                    ui.label(shown);
                } else {
                    let bad = r.regex
                        && if kind == RuleKind::User {
                            crate::rules::user_regex(&r.pattern, true, r.case_sensitive).is_none()
                        } else {
                            crate::rules::pattern_regex(&r.pattern, true, r.case_sensitive)
                                .is_none()
                        };
                    let t = ui.add(
                        egui::TextEdit::singleline(&mut r.pattern)
                            .desired_width(200.0)
                            .text_color_opt(bad.then_some(Color32::from_rgb(0xff, 0x60, 0x60))),
                    );
                    if bad {
                        t.on_hover_text("Not a valid regex");
                    }
                }
                ui.checkbox(&mut r.flash, "");
                if kind == RuleKind::Badge {
                    ui.label("");
                    ui.label("");
                } else {
                    ui.checkbox(&mut r.regex, "");
                    ui.checkbox(&mut r.case_sensitive, "");
                }
                ui.checkbox(&mut r.sound, "");
                ui.color_edit_button_srgb(&mut r.color);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(i > 0, egui::Button::new("▲").small())
                        .on_hover_text("Move up")
                        .clicked()
                    {
                        swap = Some((i, i - 1));
                    }
                    if ui
                        .add_enabled(i + 1 < n, egui::Button::new("▼").small())
                        .on_hover_text("Move down")
                        .clicked()
                    {
                        swap = Some((i, i + 1));
                    }
                });
                if ui.small_button("Remove").clicked() {
                    remove = Some(i);
                }
                ui.end_row();
            }
        });
    ui.horizontal(|ui| {
        if kind == RuleKind::Badge {
            egui::ComboBox::from_id_salt((id, "badge"))
                .selected_text(
                    BADGES
                        .iter()
                        .find(|(k, _)| *k == new_a.as_str())
                        .map(|(_, v)| *v)
                        .unwrap_or("Choose a badge"),
                )
                .show_ui(ui, |ui| {
                    for (k, v) in BADGES {
                        ui.selectable_value(new_a, k.to_string(), v);
                    }
                });
        } else {
            let hint = if kind == RuleKind::User {
                "Username"
            } else {
                "Pattern"
            };
            ui.add(
                egui::TextEdit::singleline(new_a)
                    .hint_text(hint)
                    .desired_width(300.0),
            );
        }
        if ui
            .add_enabled(!new_a.trim().is_empty(), egui::Button::new("Add"))
            .clicked()
        {
            rows.push(HighlightRule {
                pattern: new_a.trim().to_string(),
                ..Default::default()
            });
            new_a.clear();
        }
    });
    if let Some((a, b)) = swap {
        rows.swap(a, b);
    }
    if let Some(i) = remove {
        rows.remove(i);
    }
}

impl LiveApp {
    fn nicknames_page(&mut self, ui: &mut egui::Ui) {
        heading(ui, "Nicknames", TEXT);
        ui.label("Show a different name for a user in chat (Chatterino's Nicknames).");
        let s = &mut self.settings_ui;
        pair_list(
            ui,
            "nick-grid",
            ("Username", "Nickname"),
            &mut self.settings.nicknames,
            &mut s.new_a,
            &mut s.new_b,
        );
    }

    fn commands_page(&mut self, ui: &mut egui::Ui) {
        heading(ui, "Commands", TEXT);
        ui.label("Your own commands, as in Chatterino. Typing the trigger sends the text instead.");
        ui.weak("{1}, {2} ... are the words after the trigger, {1+} is everything after it, {channel} is the channel.");
        ui.weak("Example: trigger /hello, text \"Hello {1}, welcome to {channel}!\"");
        let mut rows: Vec<(String, String)> = self
            .settings
            .commands
            .iter()
            .map(|c| (c.trigger.clone(), c.text.clone()))
            .collect();
        let s = &mut self.settings_ui;
        pair_list(
            ui,
            "cmd-grid",
            ("Trigger", "Text"),
            &mut rows,
            &mut s.new_a,
            &mut s.new_b,
        );
        self.settings.commands = rows
            .into_iter()
            .map(|(trigger, text)| CustomCommand { trigger, text })
            .collect();
        let plugin_cmds = self.plugins.commands();
        if !plugin_cmds.is_empty() {
            ui.weak(format!("Commands from plugins: {}", plugin_cmds.join(", ")));
        }
    }

    /// chatterino7's Highlights: Messages | Users | Badges | Blacklisted Users.
    fn highlights_page(&mut self, ui: &mut egui::Ui) {
        sub_tabs(
            ui,
            &["Messages", "Users", "Badges", "Blacklisted Users"],
            &mut self.settings_ui.sub_tab,
        );
        let s = &mut self.settings_ui;
        match s.sub_tab {
            0 => {
                // search-tab: 0 Messages
                ui.label("Highlight messages based on certain patterns. Message highlights are prioritized over user and badge highlights.");
                ui.checkbox(
                    &mut self.settings.highlight_mentions,
                    "Highlight messages that mention my name",
                );
                ui.weak("A pattern without regex matches as a whole word; case doesn't matter unless Case-sensitive is on.");
                highlight_table(
                    ui,
                    "hl-phrases",
                    RuleKind::Message,
                    &mut self.settings.highlight_phrases,
                    &mut s.new_a,
                );
            }
            1 => {
                // search-tab: 1 Users
                ui.label("Highlight every message of these users.");
                highlight_table(
                    ui,
                    "hl-users",
                    RuleKind::User,
                    &mut self.settings.highlight_users,
                    &mut s.new_b,
                );
            }
            2 => {
                // search-tab: 2 Badges
                ui.label("Highlight messages of users with these badges (Twitch).");
                highlight_table(
                    ui,
                    "hl-badges",
                    RuleKind::Badge,
                    &mut self.settings.highlight_badges,
                    &mut s.new_a,
                );
            }
            _ => {
                // search-tab: 3 Blacklisted Users blacklist
                ui.label("Messages of these users are never highlighted (also not for mentions).");
                word_list(
                    ui,
                    "hl-blacklist",
                    "Username",
                    &mut self.settings.highlight_blacklist,
                    &mut s.new_a,
                );
            }
        }
        // search-tab: 0 sound
        ui.add_space(8.0);
        ui.label("Default sound: the Windows notification sound.");
        ui.checkbox(
            &mut self.settings.highlight_sound_focused,
            "Play highlight sound even when Yaperino is focused",
        );
        ui.weak("A highlighted message in a tab you aren't looking at marks the tab red, as in Chatterino.");
    }

    /// chatterino7's Ignores: Messages (Pattern, Regex, Case-sensitive, Block, Replacement) | Users.
    fn ignores_page(&mut self, ui: &mut egui::Ui) {
        sub_tabs(ui, &["Messages", "Users"], &mut self.settings_ui.sub_tab);
        if self.settings_ui.sub_tab == 0 {
            // search-tab: 0 Messages
            ui.label("Ignore messages based on certain patterns: Block hides the message, otherwise what matched is replaced.");
            let rows = &mut self.settings.ignore_rules;
            let mut remove: Option<usize> = None;
            egui::Grid::new("ign-rules")
                .num_columns(6)
                .striped(true)
                .spacing([10.0, 4.0])
                .show(ui, |ui| {
                    if rows.is_empty() {
                        return;
                    }
                    for h in [
                        "Pattern",
                        "Regex",
                        "Case-sensitive",
                        "Block",
                        "Replacement",
                        "",
                    ] {
                        ui.weak(h);
                    }
                    ui.end_row();
                    for (i, r) in rows.iter_mut().enumerate() {
                        let bad = r.regex
                            && crate::rules::pattern_regex(&r.pattern, true, r.case_sensitive)
                                .is_none();
                        let t = ui.add(
                            egui::TextEdit::singleline(&mut r.pattern)
                                .desired_width(200.0)
                                .text_color_opt(bad.then_some(Color32::from_rgb(0xff, 0x60, 0x60))),
                        );
                        if bad {
                            t.on_hover_text("Not a valid regex");
                        }
                        ui.checkbox(&mut r.regex, "");
                        ui.checkbox(&mut r.case_sensitive, "");
                        ui.checkbox(&mut r.block, "");
                        ui.add_enabled(
                            !r.block,
                            egui::TextEdit::singleline(&mut r.replacement)
                                .desired_width(140.0)
                                .hint_text("***"),
                        );
                        if ui.small_button("Remove").clicked() {
                            remove = Some(i);
                        }
                        ui.end_row();
                    }
                });
            let s = &mut self.settings_ui;
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut s.new_a)
                        .hint_text("Pattern")
                        .desired_width(300.0),
                );
                if ui
                    .add_enabled(!s.new_a.trim().is_empty(), egui::Button::new("Add"))
                    .clicked()
                {
                    rows.push(IgnoreRule {
                        pattern: s.new_a.trim().to_string(),
                        block: true,
                        ..Default::default()
                    });
                    s.new_a.clear();
                }
            });
            if let Some(i) = remove {
                rows.remove(i);
            }
        } else {
            // search-tab: 1 Users
            ui.label("Messages of these users are hidden (the exact name, any case).");
            let s = &mut self.settings_ui;
            word_list(
                ui,
                "ign-users",
                "Username",
                &mut self.settings.ignore_users,
                &mut s.new_b,
            );
            ui.add_space(12.0);
            ui.strong("Hidden users (/hide)");
            ui.label("Hidden only on this computer: they aren't blocked or banned, and /unhide <user> shows them again.");
            let mut show_again: Option<usize> = None;
            if self.settings.hidden_users.is_empty() {
                ui.weak("Nobody is hidden. Type /hide <user>, or right-click a message > Hide messages from ...");
            }
            for (i, u) in self.settings.hidden_users.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(u);
                    if ui.small_button("Show again").clicked() {
                        show_again = Some(i);
                    }
                });
            }
            if let Some(i) = show_again {
                self.settings.hidden_users.remove(i);
            }
        }
    }

    /// chatterino7's Filters: named filters (Name, Filter, Valid); a split picks them in its
    /// menu (Set filters).
    fn filters_page(&mut self, ui: &mut egui::Ui) {
        heading(ui, "Filters", TEXT);
        ui.label("Selectively display messages in splits using channel filters. Set filters under a split's menu (⋮ > Set filters).");
        let rows = &mut self.settings.filters;
        let mut remove: Option<usize> = None;
        egui::Grid::new("filters")
            .num_columns(4)
            .striped(true)
            .spacing([10.0, 4.0])
            .show(ui, |ui| {
                if rows.is_empty() {
                    return;
                }
                for h in ["Name", "Filter", "Valid", ""] {
                    ui.weak(h);
                }
                ui.end_row();
                for (i, f) in rows.iter_mut().enumerate() {
                    ui.add(egui::TextEdit::singleline(&mut f.name).desired_width(120.0));
                    ui.add(
                        egui::TextEdit::singleline(&mut f.filter)
                            .desired_width(300.0)
                            .font(egui::TextStyle::Monospace),
                    );
                    match crate::rules::parse_filter(&f.filter) {
                        Ok(_) => ui.label(
                            egui::RichText::new("Yes").color(Color32::from_rgb(0x60, 0xd0, 0x60)),
                        ),
                        Err(e) => ui
                            .label(
                                egui::RichText::new("No")
                                    .color(Color32::from_rgb(0xff, 0x60, 0x60)),
                            )
                            .on_hover_text(e),
                    };
                    if ui.small_button("Remove").clicked() {
                        remove = Some(i);
                    }
                    ui.end_row();
                }
            });
        let s = &mut self.settings_ui;
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut s.new_a)
                    .hint_text("Name")
                    .desired_width(120.0),
            );
            ui.add(
                egui::TextEdit::singleline(&mut s.new_b)
                    .hint_text("author.name == \"someone\"")
                    .desired_width(300.0)
                    .font(egui::TextStyle::Monospace),
            );
            if ui
                .add_enabled(
                    !s.new_a.trim().is_empty() && !s.new_b.trim().is_empty(),
                    egui::Button::new("Add"),
                )
                .clicked()
            {
                rows.push(FilterDef {
                    name: s.new_a.trim().to_string(),
                    filter: s.new_b.trim().to_string(),
                });
                s.new_a.clear();
                s.new_b.clear();
            }
        });
        if let Some(i) = remove {
            let name = rows.remove(i).name;
            for list in self.settings.split_filters.values_mut() {
                list.retain(|n| *n != name);
            }
        }
        ui.checkbox(
            &mut self.settings.filters_skip_own,
            "Do not filter my own messages",
        );
        ui.collapsing("filter info", |ui| {
            ui.label("Values: author.name, author.badges, message.content, message.emote_only (true when the message is only emotes), channel.name, platform (Twitch, Kick, YouTube, TikTok).");
            ui.label(egui::RichText::new("!message.emote_only").monospace());
            ui.label("Compare with ==, !=, contains, startswith, endswith, matches (a regex); join with && (and), || (or), ! (not) and brackets. Text goes in quotes; case doesn't matter.");
            ui.label(egui::RichText::new(r#"message.content contains "giveaway" || author.badges contains "moderator""#).monospace());
            ui.label(egui::RichText::new(r#"!(platform == "kick") && author.name != "nightbot""#).monospace());
        });
    }

    /// chatterino7's Hotkeys: every shortcut by category, with a search; "Change" records
    /// new keys (Esc cancels), "Reset" goes back to Chatterino's default.
    fn hotkeys_page(&mut self, ui: &mut egui::Ui) {
        // recording: the next key pressed (with Ctrl / Shift / Alt) becomes the shortcut
        if let Some(id) = self.settings_ui.recording {
            if let Some((m, key)) = crate::hotkeys::recorded(ui.ctx()) {
                if key != egui::Key::Escape {
                    self.settings
                        .hotkeys
                        .insert(id.to_string(), crate::hotkeys::format(m, key));
                }
                self.settings_ui.recording = None;
            }
        }
        ui.horizontal(|ui| {
            ui.label("Search keybind:");
            ui.add(
                egui::TextEdit::singleline(&mut self.settings_ui.hotkey_search)
                    .hint_text("Name or keys")
                    .desired_width(220.0),
            );
            if ui.button("Reset to defaults").clicked() {
                self.settings.hotkeys.clear();
            }
        });
        ui.add_space(4.0);
        let q = self.settings_ui.hotkey_search.trim().to_lowercase();
        let mut change: Option<&'static str> = None;
        let mut reset: Option<&'static str> = None;
        let mut clear: Option<&'static str> = None;
        egui::Grid::new("hotkeys")
            .num_columns(3)
            .striped(true)
            .spacing([24.0, 5.0])
            .min_col_width(160.0)
            .show(ui, |ui| {
                ui.weak("Hotkey name");
                ui.weak("Keybinding");
                ui.label("");
                ui.end_row();
                let mut last_cat = "";
                for (id, cat, name, default) in crate::hotkeys::ACTIONS {
                    let keys = crate::hotkeys::keys_of(id, &self.settings.hotkeys).to_string();
                    if !(q.is_empty() || name.contains(&q) || keys.to_lowercase().contains(&q)) {
                        continue;
                    }
                    if cat != last_cat {
                        ui.label(
                            egui::RichText::new(cat)
                                .size(16.0)
                                .color(crate::style::theme::c().strong_text),
                        );
                        ui.label("");
                        ui.label("");
                        ui.end_row();
                        last_cat = cat;
                    }
                    ui.label(name);
                    if self.settings_ui.recording == Some(id) {
                        ui.label(
                            egui::RichText::new("Press the new keys... (Esc cancels)")
                                .color(crate::style::theme::c().accent),
                        );
                    } else {
                        // two actions on the same keys: the first one wins, so say so
                        let clash = !keys.is_empty()
                            && crate::hotkeys::ACTIONS.iter().any(|(o, _, _, _)| {
                                o != &id
                                    && crate::hotkeys::keys_of(o, &self.settings.hotkeys) == keys
                            });
                        let shown = if keys.is_empty() {
                            "(none)".to_string()
                        } else {
                            keys.clone()
                        };
                        let l = ui.label(egui::RichText::new(shown).monospace().color(if clash {
                            Color32::from_rgb(0xff, 0x60, 0x60)
                        } else {
                            crate::style::theme::c().text
                        }));
                        if clash {
                            l.on_hover_text("Another hotkey uses these keys too");
                        }
                    }
                    ui.horizontal(|ui| {
                        if ui.small_button("Change").clicked() {
                            change = Some(id);
                        }
                        if ui
                            .add_enabled(keys != default, egui::Button::new("Reset").small())
                            .clicked()
                        {
                            reset = Some(id);
                        }
                        if ui
                            .add_enabled(!keys.is_empty(), egui::Button::new("Remove").small())
                            .clicked()
                        {
                            clear = Some(id);
                        }
                    });
                    ui.end_row();
                }
                ui.label(
                    egui::RichText::new("Split input")
                        .size(16.0)
                        .color(crate::style::theme::c().strong_text),
                );
                ui.label("");
                ui.label("");
                ui.end_row();
                for (name, keys) in crate::hotkeys::FIXED {
                    if q.is_empty() || name.contains(&q) || keys.to_lowercase().contains(&q) {
                        ui.label(name);
                        ui.label(egui::RichText::new(keys).monospace());
                        ui.weak("fixed");
                        ui.end_row();
                    }
                }
            });
        if let Some(id) = change {
            self.settings_ui.recording = Some(id);
        }
        if let Some(id) = reset {
            self.settings.hotkeys.remove(id);
        }
        if let Some(id) = clear {
            self.settings.hotkeys.insert(id.to_string(), String::new());
        }
    }
    /// chatterino7's Moderation > Logs: chat logs as text files.
    fn moderation_page(&mut self, ui: &mut egui::Ui) {
        sub_tabs(
            ui,
            &["Logs", "Moderation buttons", "User Timeout Buttons"],
            &mut self.settings_ui.sub_tab,
        );
        let can = self.account.as_ref().is_some_and(|a| a.can_moderate());
        if self.settings_ui.sub_tab == 1 {
            ui.label("Moderation in Twitch channels you moderate: right-click a message > Moderation (timeout, ban, unban, delete message).");
            ui.checkbox(
                &mut self.settings.mod_buttons,
                "Show moderation buttons before each message (delete, timeout, ban)",
            );
            ui.weak("The timeout button uses 10 minutes, or the first of your timeout buttons when 10 minutes isn't one of them.");
            if !can {
                ui.colored_label(Color32::from_rgb(230, 160, 80), "Your Twitch sign-in doesn't have the moderation permissions yet: sign in again in Accounts to use these.");
            }
            ui.add_space(8.0);
            ui.strong("Mod commands");
            ui.label("Typed in a Twitch split, they run through Twitch's API (split menu > Moderation has the chat modes). Mod commands work in channels you moderate; broadcaster commands in your own channel.");
            ui.label(egui::RichText::new(crate::moderation::HELP).weak());
            if let Some(missing) = self
                .account
                .as_ref()
                .map(|a| a.missing_scopes())
                .filter(|m| !m.is_empty())
            {
                ui.colored_label(
                    Color32::from_rgb(230, 160, 80),
                    format!("Sign in again in Accounts to allow: {}", missing.join(", ")),
                );
            }
            return;
        }
        if self.settings_ui.sub_tab == 2 {
            ui.label("The timeouts offered in a message's menu (Moderation > Timeout): a number with s, m, h, d or w (1 second to 2 weeks).");
            let s = &mut self.settings_ui;
            word_list(
                ui,
                "timeout-buttons",
                "e.g. 10m",
                &mut self.settings.timeout_buttons,
                &mut s.new_a,
            );
            let bad: Vec<&String> = self
                .settings
                .timeout_buttons
                .iter()
                .filter(|t| crate::moderation::parse_duration(t).is_none())
                .collect();
            if !bad.is_empty() {
                ui.colored_label(
                    Color32::from_rgb(0xff, 0x60, 0x60),
                    format!(
                        "Not a duration: {}",
                        bad.iter()
                            .map(|s| s.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                );
            }
            if ui.button("Reset to Chatterino's").clicked() {
                self.settings.timeout_buttons = ["1s", "30s", "1m", "5m", "30m", "1h", "1d", "1w"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
            }
            return;
        }
        ui.checkbox(&mut self.settings.logging, "Enable logging");
        let dir = if self.settings.log_dir.trim().is_empty() {
            super::features::default_log_dir()
        } else {
            PathBuf::from(self.settings.log_dir.trim())
        };
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.label("Logs are saved at ");
            let shown = dir.display().to_string();
            if ui.link(&shown).clicked() {
                let _ = std::fs::create_dir_all(&dir);
                browser::open_url(&shown);
            }
            ui.label(". They are saved as plain text files per channel and day, with the time of each message.");
        });
        ui.horizontal(|ui| {
            ui.label("Log folder");
            ui.add(
                egui::TextEdit::singleline(&mut self.settings.log_dir)
                    .hint_text(super::features::default_log_dir().display().to_string())
                    .desired_width(340.0),
            );
            if ui
                .add_enabled(
                    !self.settings.log_dir.is_empty(),
                    egui::Button::new("Reset"),
                )
                .clicked()
            {
                self.settings.log_dir.clear();
            }
        });
        let size = *self
            .settings_ui
            .log_size
            .get_or_insert_with(|| super::features::folder_size(&dir));
        ui.label(format!(
            "Your logs currently take up {} of space",
            super::features::human_size(size)
        ));
        ui.horizontal(|ui| {
            ui.label("Log file timestamp format");
            egui::ComboBox::from_id_salt("log-format")
                .selected_text(if self.settings.log_seconds {
                    "hh:mm:ss"
                } else {
                    "hh:mm"
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.settings.log_seconds, true, "hh:mm:ss");
                    ui.selectable_value(&mut self.settings.log_seconds, false, "hh:mm");
                });
        });
        ui.checkbox(
            &mut self.settings.log_only_listed,
            "Only log channels listed below",
        );
        let s = &mut self.settings_ui;
        word_list(
            ui,
            "log-channels",
            "Channel (e.g. xqc or kick:name)",
            &mut self.settings.log_channels,
            &mut s.new_a,
        );
    }

    /// chatterino7's Live Notifications: Options | Selected Channels.
    fn notifications_page(&mut self, ui: &mut egui::Ui) {
        sub_tabs(
            ui,
            &["Options", "Selected Channels"],
            &mut self.settings_ui.sub_tab,
        );
        if self.settings_ui.sub_tab == 0 {
            ui.label("You can be informed when certain channels go live. You can be informed in multiple ways:");
            ui.checkbox(&mut self.settings.notify_flash, "Flash taskbar");
            ui.checkbox(
                &mut self.settings.notify_sound_selected,
                "Play sound for selected channels",
            );
            ui.checkbox(
                &mut self.settings.notify_sound_all,
                "Play sound for any channel going live",
            );
            ui.checkbox(
                &mut self.settings.notify_suppress_startup,
                "Suppress live notifications on startup",
            );
            ui.checkbox(&mut self.settings.notify_toast, "Show notification");
            ui.add_enabled_ui(self.settings.notify_toast, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    ui.label("Action when clicking on a notification:");
                    egui::ComboBox::from_id_salt("notify-click")
                        .selected_text(self.settings.notify_click.clone())
                        .show_ui(ui, |ui| {
                            for a in [
                                "Open chat tab",
                                "Open in browser",
                                "Open in Streamlink",
                                "Do nothing",
                            ] {
                                ui.selectable_value(
                                    &mut self.settings.notify_click,
                                    a.to_string(),
                                    a,
                                );
                            }
                        });
                });
            });
            if ui.button("Show a test notification").clicked() {
                let _ = super::features::show_toast(
                    "Yaperino",
                    "This is how a live notification looks.",
                    || {},
                );
            }
            ui.checkbox(
                &mut self.settings.notify_all,
                "Notify for every followed channel (a message in its chat and here)",
            );
            ui.weak("Sound: the Windows notification sound. Selected channels are on the next tab (also: a split's ⋮ menu > Notify when live).");
        } else {
            ui.label("Channels you are notified about when they go live:");
            let s = &mut self.settings_ui;
            word_list(
                ui,
                "notify-list",
                "Channel",
                &mut self.settings.notify_channels,
                &mut s.new_a,
            );
        }
    }

    /// chatterino7's Plugins: general plugin settings, then one box per plugin.
    fn plugins_page(&mut self, ui: &mut egui::Ui) {
        let mut reload_all = false;
        let mut toggle: Option<String> = None;
        let mut reload: Option<String> = None;
        about_box(ui, "General plugin settings", |ui| {
            ui.label("Plugins can expand what Yaperino does. They are made in Lua, as Chatterino's plugins. Use ONLY the plugins you trust.");
            ui.label("Yaperino supports Chatterino's API for commands (c2.register_command, channel:send_message, channel:add_system_message, c2.system_msg, c2.log, c2.later), HTTP requests (c2.HTTPRequest, with the Network permission) and files in the plugin's own data folder (with the Filesystem permissions). Plugins can never start programs or touch other files.");
            if let Some(dir) = crate::plugins::folder() {
                let shown = dir.display().to_string();
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label("You can load plugins by putting them into ");
                    if ui
                        .link("the Plugins directory")
                        .on_hover_text(&shown)
                        .clicked()
                    {
                        let _ = std::fs::create_dir_all(&dir);
                        browser::open_url(&shown);
                    }
                    ui.label(". Each one is a new directory with info.json and init.lua.");
                });
            }
            if ui
                .checkbox(&mut self.settings.plugins_enabled, "Enable plugins")
                .changed()
            {
                reload_all = true;
            }
            if ui.button("Look for plugins again").clicked() {
                reload_all = true;
            }
        });
        if self.plugin_list.is_empty() {
            ui.add_space(6.0);
            ui.weak("No plugins in the Plugins directory yet.");
        }
        for p in self.plugin_list.clone() {
            let on = self.settings.enabled_plugins.contains(&p.id);
            let running = self.plugins.is_loaded(&p.id);
            let title = format!(
                "{} ({}{}from {})",
                p.name,
                p.version,
                if p.version.is_empty() { "" } else { ", " },
                p.id
            );
            about_box(ui, &title, |ui| {
                if !p.description.is_empty() {
                    ui.label(&p.description);
                }
                egui::Grid::new(("plugin", p.id.as_str()))
                    .num_columns(2)
                    .spacing([20.0, 4.0])
                    .show(ui, |ui| {
                        ui.label("Authors");
                        ui.label(p.authors.join(", "));
                        ui.end_row();
                        ui.label("Homepage");
                        if p.homepage.is_empty() {
                            ui.label("");
                        } else if ui.link(&p.homepage).clicked() {
                            browser::open_url(&p.homepage);
                        }
                        ui.end_row();
                        ui.label("License");
                        ui.label(&p.license);
                        ui.end_row();
                        ui.label("Commands");
                        ui.label(self.plugins.commands_of(&p.id).join(", "));
                        ui.end_row();
                    });
                if !p.permissions.is_empty() {
                    ui.label("Required permissions:");
                    for perm in &p.permissions {
                        ui.label(format!("   • {}", crate::plugins::permission_text(perm)));
                    }
                }
                if let Some(problem) = &p.problem {
                    ui.colored_label(Color32::from_rgb(0xff, 0x60, 0x60), problem);
                }
                ui.weak(if running {
                    "Running"
                } else if on && !self.settings.plugins_enabled {
                    "Enabled; turn on \"Enable plugins\" above"
                } else if on {
                    "Enabled, not running (see the log below)"
                } else {
                    "Not enabled"
                });
                let w = ui.available_width();
                if ui
                    .add_enabled(
                        p.problem.is_none(),
                        egui::Button::new(if on { "Disable" } else { "Enable" })
                            .min_size(Vec2::new(w, 24.0)),
                    )
                    .clicked()
                {
                    toggle = Some(p.id.clone());
                }
                if ui
                    .add_enabled(
                        on && self.settings.plugins_enabled,
                        egui::Button::new("Reload").min_size(Vec2::new(w, 24.0)),
                    )
                    .clicked()
                {
                    reload = Some(p.id.clone());
                }
            });
        }
        if !self.plugins.log.is_empty() {
            ui.collapsing("Plugin log", |ui| {
                for l in self.plugins.log.iter().rev().take(50) {
                    ui.label(egui::RichText::new(l).monospace().size(11.0));
                }
            });
        }
        if let Some(id) = toggle {
            if let Some(i) = self.settings.enabled_plugins.iter().position(|x| *x == id) {
                self.settings.enabled_plugins.remove(i);
                self.plugins.unload(&id);
            } else {
                self.settings.enabled_plugins.push(id);
                reload_all = true;
            }
        }
        if let Some(id) = reload {
            self.plugins.unload(&id);
            reload_all = true;
        }
        if reload_all {
            self.load_plugins();
        }
    }
}

/// Open-source software Yaperino is built with (name, home page, license).
const OPEN_SOURCE: [(&str, &str, &str); 18] = [
    (
        "egui / eframe",
        "https://github.com/emilk/egui",
        "MIT OR Apache-2.0",
    ),
    (
        "egui_extras",
        "https://github.com/emilk/egui",
        "MIT OR Apache-2.0",
    ),
    (
        "winit",
        "https://github.com/rust-windowing/winit",
        "Apache-2.0",
    ),
    (
        "wry (WebView2)",
        "https://github.com/tauri-apps/wry",
        "MIT OR Apache-2.0",
    ),
    (
        "ureq",
        "https://github.com/algesten/ureq",
        "MIT OR Apache-2.0",
    ),
    (
        "tungstenite",
        "https://github.com/snapview/tungstenite-rs",
        "MIT OR Apache-2.0",
    ),
    (
        "rustls",
        "https://github.com/rustls/rustls",
        "MIT OR Apache-2.0 OR ISC",
    ),
    (
        "serde / serde_json",
        "https://github.com/serde-rs/serde",
        "MIT OR Apache-2.0",
    ),
    (
        "image",
        "https://github.com/image-rs/image",
        "MIT OR Apache-2.0",
    ),
    (
        "flate2",
        "https://github.com/rust-lang/flate2-rs",
        "MIT OR Apache-2.0",
    ),
    (
        "sha1 / sha2",
        "https://github.com/RustCrypto/hashes",
        "MIT OR Apache-2.0",
    ),
    (
        "getrandom",
        "https://github.com/rust-random/getrandom",
        "MIT OR Apache-2.0",
    ),
    (
        "winresource",
        "https://github.com/BenjaminRi/winresource",
        "MIT",
    ),
    (
        "Streamlink (optional)",
        "https://streamlink.github.io",
        "BSD-2-Clause",
    ),
    (
        "notify-rust (macOS, Linux)",
        "https://github.com/hoodie/notify-rust",
        "MIT OR Apache-2.0",
    ),
    (
        "Noto Emoji font (macOS, Linux)",
        "https://github.com/googlefonts/noto-emoji",
        "OFL-1.1",
    ),
    (
        "spellbook (spellcheck)",
        "https://github.com/helix-editor/spellbook",
        "MPL-2.0",
    ),
    (
        "English dictionary (SCOWL, via wooorm/dictionaries)",
        "http://wordlist.aspell.net",
        "MIT AND BSD (assets/dictionaries/en_US-LICENSE.txt)",
    ),
];

/// A spellcheck language chip: its name, a status mark and ✕, drawn as one piece of known
/// size so a wrapping row can move it to the next line whole. True when ✕ was clicked.
fn lang_chip(ui: &mut egui::Ui, name: &str, mark: &str, tip: &str, fill: Color32) -> bool {
    let p = crate::style::theme::c();
    let font = egui::FontId::proportional(13.5);
    let small = egui::FontId::proportional(11.5);
    let name_g = ui
        .painter()
        .layout_no_wrap(name.to_string(), font, p.strong_text);
    let mark_g = ui
        .painter()
        .layout_no_wrap(mark.to_string(), small.clone(), p.accent);
    let x_w = 16.0;
    let mark_w = if mark.is_empty() {
        0.0
    } else {
        mark_g.size().x + 4.0
    };
    let size = Vec2::new(10.0 + name_g.size().x + 6.0 + mark_w + x_w + 4.0, 26.0);
    let (rect, chip) = ui.allocate_exact_size(size, Sense::hover());
    let x_rect = Rect::from_min_size(
        Pos2::new(rect.right() - x_w - 4.0, rect.top()),
        Vec2::new(x_w, rect.height()),
    );
    let x = ui.interact(x_rect, chip.id.with("remove"), Sense::click());
    let painter = ui.painter();
    painter.rect_filled(rect, 13.0, fill);
    let mid = rect.center().y;
    painter.galley(
        Pos2::new(rect.left() + 10.0, mid - name_g.size().y / 2.0),
        name_g.clone(),
        p.strong_text,
    );
    if !mark.is_empty() {
        painter.galley(
            Pos2::new(
                rect.left() + 10.0 + name_g.size().x + 6.0,
                mid - mark_g.size().y / 2.0,
            ),
            mark_g,
            p.accent,
        );
    }
    painter.text(
        x_rect.center(),
        egui::Align2::CENTER_CENTER,
        "✕",
        small,
        if x.hovered() { p.strong_text } else { p.system },
    );
    chip.on_hover_text(tip);
    x.on_hover_text("Remove").clicked()
}

/// A titled box like Chatterino's About page sections.
fn about_box(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new(title)
            .font(FontId::new(13.0, crate::style::bold()))
            .color(crate::style::theme::c().strong_text),
    );
    ui.add_space(3.0);
    egui::Frame::new()
        .stroke(egui::Stroke::new(1.0, crate::style::theme::c().shade(28)))
        .inner_margin(egui::Margin::symmetric(18, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 6.0;
            add(ui);
        });
}

/// "text <link> text" on one line, the link opening in the browser (or a folder).
fn link_line(ui: &mut egui::Ui, before: &str, link: &str, target: &str, after: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label(before);
        if ui.link(link).on_hover_text(target).clicked() {
            crate::browser::open_url(target);
        }
        ui.label(after);
    });
}

/// Linux or macOS, and the processor type, e.g. "macOS (aarch64)".
fn other_system() -> String {
    let name = match std::env::consts::OS {
        "macos" => "macOS",
        "linux" => "Linux",
        other => other,
    };
    format!("{name} ({})", std::env::consts::ARCH)
}

/// The Windows version, e.g. "Windows 11 Version 26H2, kernel 10.0.26300" (asked once).
fn windows_version() -> String {
    static V: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    V.get_or_init(|| {
        let q = |name: &str| -> String {
            let mut cmd = std::process::Command::new("reg");
            cmd.args([
                "query",
                r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion",
                "/v",
                name,
            ]);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x0800_0000);
            }
            cmd.output()
                .ok()
                .and_then(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .find(|l| l.contains(name))
                        .and_then(|l| l.split("REG_SZ").nth(1))
                        .map(|v| v.trim().to_string())
                })
                .unwrap_or_default()
        };
        let build: u32 = q("CurrentBuildNumber").parse().unwrap_or(0);
        let name = if build >= 22000 {
            "Windows 11".to_string()
        } else {
            q("ProductName")
        };
        let display = q("DisplayVersion");
        format!(
            "{name}{}{}",
            if display.is_empty() {
                String::new()
            } else {
                format!(" Version {display}")
            },
            if build > 0 {
                format!(", kernel: 10.0.{build}")
            } else {
                String::new()
            }
        )
    })
    .clone()
}

impl LiveApp {
    /// The logo picture (cached).
    pub(super) fn logo_texture(&mut self, ctx: &egui::Context) -> egui::TextureHandle {
        self.settings_ui
            .logo
            .get_or_insert_with(|| {
                let img =
                    egui::ColorImage::from_rgba_unmultiplied([128, 128], &crate::logo::rgba(128));
                ctx.load_texture("yaperino-logo", img, egui::TextureOptions::LINEAR)
            })
            .clone()
    }

    /// About, laid out like Chatterino's About page: the logo banner, then boxes for the
    /// version, about Yaperino, installing, and the open-source software used.
    fn about_page(&mut self, ui: &mut egui::Ui) {
        let tex = self.logo_texture(ui.ctx());
        // the banner: logo, a light wordmark, and lines running out to the right
        ui.add_space(8.0);
        let (rect, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width().min(560.0), 110.0),
            Sense::hover(),
        );
        let p = ui.painter();
        let logo = egui::Rect::from_min_size(rect.min + Vec2::new(0.0, 4.0), Vec2::splat(100.0));
        let accent = crate::style::theme::c().accent;
        for i in 0..9 {
            let y = logo.center().y - 34.0 + i as f32 * 8.5;
            let fade = 1.0 - (i as f32 - 4.0).abs() / 6.0;
            p.hline(
                (logo.right() - 20.0)..=rect.right(),
                y,
                egui::Stroke::new(1.0, accent.gamma_multiply(0.18 * fade)),
            );
        }
        p.image(
            tex.id(),
            logo,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        p.text(
            egui::Pos2::new(logo.right() + 16.0, logo.center().y),
            egui::Align2::LEFT_CENTER,
            "YAPERINO",
            FontId::proportional(46.0),
            crate::style::theme::c().strong_text,
        );

        about_box(ui, "Version", |ui| {
            ui.label(format!(
                "Yaperino {} built with Rust and egui",
                crate::VERSION
            ));
            let system = if cfg!(windows) {
                windows_version()
            } else {
                other_system()
            };
            ui.label(format!("Running on {system}"));
            ui.add_space(4.0);
            if let Some(dir) = crate::paths::app_dir() {
                let shown = dir.display().to_string();
                link_line(
                    ui,
                    "Your settings directory is located at ",
                    &shown,
                    &shown,
                    ".",
                );
            }
        });
        about_box(ui, "About Yaperino...", |ui| {
            ui.label("Chat for Twitch, Kick, YouTube and TikTok in tabs and splits, with a live list of the channels you follow (Alt+L).");
            link_line(
                ui,
                "Yaperino works like Chatterino: the Chatterino Wiki can be found ",
                "here",
                "https://wiki.chatterino.com",
                "",
            );
            link_line(
                ui,
                "All about Chatterino's ",
                "features",
                "https://chatterino.com/#features",
                "",
            );
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.label("Built after ");
                for (i, (name, url)) in [
                    ("Chatterino 2", "https://github.com/Chatterino/chatterino2"),
                    (
                        "chatterino7 (7TV)",
                        "https://github.com/SevenTV/chatterino7",
                    ),
                    ("Moltorino", "https://github.com/EEVA-inc/Moltorino"),
                    ("Chatty", "https://github.com/chatty/chatty"),
                ]
                .iter()
                .enumerate()
                {
                    if ui.link(*name).on_hover_text(*url).clicked() {
                        crate::browser::open_url(url);
                    }
                    ui.label(match i {
                        0 | 1 => ", ",
                        2 => " and ",
                        _ => ".",
                    });
                }
            });
        });
        // Install / Uninstall (Start menu and Apps list) is Windows only
        if cfg!(windows) {
            let installed = *self
                .settings_ui
                .installed
                .get_or_insert_with(crate::install::is_installed);
            let mut do_install = false;
            let mut do_uninstall = false;
            let message = self.settings_ui.install_message.clone();
            about_box(ui, "Install", |ui| {
                if installed {
                    ui.label("Yaperino is installed for you: it is in Start and in Settings > Apps > Installed apps.");
                } else {
                    ui.label("Put Yaperino in Start and in Settings > Apps > Installed apps (for you only, no admin needed). Your settings and sign-ins stay the same.");
                }
                ui.horizontal(|ui| {
                    if ui
                        .button(if installed {
                            "Update the installed Yaperino"
                        } else {
                            "Install Yaperino"
                        })
                        .clicked()
                    {
                        do_install = true;
                    }
                    if installed && ui.button("Uninstall").clicked() {
                        do_uninstall = true;
                    }
                });
                if !message.is_empty() {
                    ui.label(&message);
                }
                if let Some(d) = crate::install::install_dir() {
                    ui.weak(format!("Installed to {}", d.display()));
                }
            });
            if do_install {
                self.settings_ui.install_message = crate::install::install().unwrap_or_else(|e| e);
                self.settings_ui.installed = None;
            }
            if do_uninstall {
                self.settings_ui.install_message = match crate::install::uninstall() {
                    Ok(()) => {
                        "Removed from Start and the Apps list. Settings and sign-ins are kept."
                            .into()
                    }
                    Err(e) => e,
                };
                self.settings_ui.installed = None;
            }
        }
        // Moltorino's Settings Manager
        let mut backup_now = false;
        let mut restore: Option<PathBuf> = None;
        let backups = super::backup::list_backups();
        let message = self.settings_ui.backup_message.clone();
        let auto = &mut self.settings.auto_backup;
        let import = &mut self.settings_ui.backup_import;
        about_box(ui, "Settings backup", |ui| {
            ui.label("Back up your settings, or go back to an earlier backup. Backups never hold sign-ins: your accounts stay as they are.");
            ui.checkbox(
                auto,
                "Back up automatically once a day (the last 10 are kept)",
            );
            ui.horizontal(|ui| {
                if ui.button("Back up now").clicked() {
                    backup_now = true;
                }
                if ui.button("Open the backups folder").clicked() {
                    let dir = super::backup::backup_dir();
                    let _ = std::fs::create_dir_all(&dir);
                    crate::browser::open_url(&dir.display().to_string());
                }
            });
            if !message.is_empty() {
                ui.label(&message);
            }
            if !backups.is_empty() {
                ui.add_space(4.0);
                ui.label("Restore a backup (the current settings are backed up first):");
                for b in backups.iter().take(10) {
                    ui.horizontal(|ui| {
                        let name = b.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        ui.monospace(
                            name.trim_start_matches("yaperino-settings-")
                                .trim_end_matches(".json"),
                        );
                        if ui.small_button("Restore").clicked() {
                            restore = Some(b.clone());
                        }
                    });
                }
            }
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(import)
                        .hint_text("Or a backup file's full path...")
                        .desired_width(300.0),
                );
                if ui.button("Import").clicked() && !import.trim().is_empty() {
                    restore = Some(PathBuf::from(import.trim()));
                }
            });
        });
        if backup_now {
            self.settings_ui.backup_message =
                match super::backup::write_backup(&self.settings, false) {
                    Ok(p) => format!("Backed up to {}", p.display()),
                    Err(e) => format!("Backup failed: {e}"),
                };
        }
        if let Some(path) = restore {
            let result = std::fs::read_to_string(&path)
                .map_err(|e| format!("{}: {e}", path.display()))
                .and_then(|text| super::backup::from_backup(&self.settings, &text));
            self.settings_ui.backup_message = match result {
                Ok(restored) => {
                    let _ = super::backup::write_backup(&self.settings, false);
                    self.settings = restored;
                    self.settings_ui.backup_import.clear();
                    "Restored. The settings from before are in a new backup.".to_string()
                }
                Err(e) => format!("Not restored: {e}"),
            };
        }
        let shown = &mut self.settings_ui.shown_licenses;
        about_box(ui, "Open source software used...", |ui| {
            egui::Grid::new("open-source")
                .num_columns(2)
                .spacing([40.0, 6.0])
                .show(ui, |ui| {
                    for (i, (name, url, license)) in OPEN_SOURCE.iter().enumerate() {
                        if ui.link(*name).on_hover_text(*url).clicked() {
                            crate::browser::open_url(url);
                        }
                        let open = shown.contains(&i);
                        if ui
                            .link(if open {
                                format!("license: {license}")
                            } else {
                                "show license".to_string()
                            })
                            .clicked()
                        {
                            if open {
                                shown.retain(|x| *x != i);
                            } else {
                                shown.push(i);
                            }
                        }
                        ui.end_row();
                    }
                });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_search_matches_every_word_in_any_order() {
        assert!(page_matches(Page::General, ""));
        assert!(page_matches(Page::General, "spellcheck"));
        assert!(page_matches(Page::General, "completion tab"));
        assert!(page_matches(Page::General, "tab completion"));
        assert!(page_matches(Page::About, "backup"));
        assert!(page_matches(Page::Hotkeys, "hotkeys"));
        assert!(!page_matches(Page::Hotkeys, "spellcheck"));
        assert!(!page_matches(Page::General, "spellcheck nonsenseword"));
    }

    /// Where a search lands: the first page that has it.
    fn lands(q: &str) -> Option<Page> {
        Page::ALL.into_iter().find(|p| page_matches(*p, q))
    }

    #[test]
    fn settings_search_finds_what_the_pages_show() {
        // labels that were in no keyword list
        assert_eq!(lands("thinner"), Some(Page::General));
        assert_eq!(lands("lock tab layout"), Some(Page::General));
        assert_eq!(lands("padlocks"), Some(Page::General));
        assert_eq!(lands("pop out"), Some(Page::Hotkeys));
        // a word's start, not any part of it: "lock" isn't Ignores' "Block"
        assert!(!page_matches(Page::Ignores, "lock"));
        assert_eq!(lands("zzqx"), None);
    }

    #[test]
    fn settings_search_lands_on_the_heading_and_sub_tab() {
        let s = first_spot(Page::General, "accent").unwrap();
        assert_eq!(s.heading, "Accent colour");
        let s = first_spot(Page::Highlights, "blacklisted").unwrap();
        assert_eq!(s.sub_tab, 3);
        let s = first_spot(Page::Ignores, "hidden users").unwrap();
        assert_eq!(s.sub_tab, 1);
    }

    #[test]
    fn every_heading_is_found_on_its_page() {
        let all = spots();
        assert!(all.iter().filter(|s| !s.heading.is_empty()).count() > 30);
        for s in all.iter().filter(|s| !s.heading.is_empty()) {
            assert_eq!(
                first_spot(s.page, s.heading).map(|f| f.page),
                Some(s.page),
                "{:?} isn't found on {}",
                s.heading,
                s.page.label()
            );
            assert!(page_matches(s.page, s.heading));
        }
    }
}
