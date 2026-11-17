//! Moltorino's mod tools on Twitch's official data, the window side: AutoMod's held
//! messages in the chat, unban requests (/unbanrequests), the mod log (/modlogs) and the
//! pinned message above the chat. The connection is eventsub.rs, the saved log modlog.rs.
//! A child of `app`.

use super::*;
use crate::eventsub::{EsAuth, EsCmd, EsEvent, EventSub};
use crate::moderation::{ModAuth, Pin, UnbanRequest};

/// How often a channel's pinned message is asked for (Twitch has no event for pins).
const PIN_EVERY: Duration = Duration::from_secs(60);

/// The unban requests window: its channel, the requests, and the notes being typed.
pub struct UnbanWindow {
    pub channel: String,
    pub items: Vec<UnbanRequest>,
    pub loading: bool,
    pub error: String,
    pub notes: HashMap<String, String>,
}

/// The mod log window: its channel and filter.
pub struct ModLogWindow {
    pub channel: String,
    pub filter: String,
}

/// "Nov 17, 2026 20:15" in local time, from Twitch's RFC 3339 time.
fn local_time(ts: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(ts)
        .map(|t| {
            t.with_timezone(&chrono::Local)
                .format("%b %-d, %Y %H:%M")
                .to_string()
        })
        .unwrap_or_default()
}

impl LiveApp {
    /// The Twitch channel (login) a tab's chat is in.
    pub(super) fn twitch_of(&self, group: &str) -> String {
        self.chats
            .iter()
            .find(|t| t.login == group)
            .and_then(|t| t.twitch.clone())
            .unwrap_or_else(|| tab_primary(group).1)
    }

    /// The tabs showing a Twitch channel's chat.
    fn groups_showing(&self, channel: &str) -> Vec<String> {
        self.chats
            .iter()
            .map(|t| t.login.clone())
            .filter(|g| self.twitch_of(g).eq_ignore_ascii_case(channel))
            .collect()
    }

    fn mod_auth(&self) -> Option<ModAuth> {
        self.account.as_ref().map(|a| ModAuth {
            client_id: a.client_id.clone(),
            token: a.token.clone(),
            user_id: a.user_id.clone(),
        })
    }

    /// Each frame: the EventSub connection follows the channels you moderate that have an
    /// open chat (and the current sign-in), and their pinned messages stay fresh.
    pub(super) fn sync_mod_tools(&mut self) {
        if self.shot.is_some() {
            self.shot_mod_tools();
            return;
        }
        if self.demo {
            return;
        }
        let auth = self.account.as_ref().map(|a| EsAuth {
            client_id: a.client_id.clone(),
            token: a.token.clone(),
            user_id: a.user_id.clone(),
            scopes: a.scopes.clone(),
        });
        let mut want: HashMap<String, String> = HashMap::new();
        if auth.is_some() {
            for t in &self.chats {
                let ch = self.twitch_of(&t.login);
                if self.mod_in.contains(&ch) {
                    if let Some(id) = self.rooms.get(&ch) {
                        want.insert(ch, id.clone());
                    }
                }
            }
        }
        if self.eventsub.is_none() && !want.is_empty() {
            let (tx, ctx) = (self.side_tx.clone(), self.ctx.clone());
            self.eventsub = Some(EventSub::start(move |e| {
                let _ = tx.send(SideMsg::EventSub(e));
                ctx.request_repaint();
            }));
            self.es_auth = None;
            self.es_watched.clear();
        }
        if let Some(es) = &self.eventsub {
            if auth != self.es_auth {
                es.send(EsCmd::Auth(auth.clone()));
                self.es_auth = auth;
            }
            for (ch, id) in &want {
                if !self.es_watched.contains(ch) {
                    es.send(EsCmd::Watch(ch.clone(), id.clone()));
                }
            }
            for ch in self.es_watched.iter().filter(|c| !want.contains_key(*c)) {
                es.send(EsCmd::Unwatch(ch.clone()));
            }
            self.es_watched = want.keys().cloned().collect();
        }
        // pins: asked for every minute
        if let Some(a) = self.mod_auth() {
            for (ch, id) in want {
                let fresh = self
                    .pins
                    .get(&ch)
                    .is_some_and(|(_, t)| t.elapsed() < PIN_EVERY);
                if fresh || !self.pins_asked.insert(ch.clone()) {
                    continue;
                }
                let (tx, ctx, a) = (self.side_tx.clone(), self.ctx.clone(), a.clone());
                std::thread::spawn(move || {
                    let pin = crate::moderation::get_pin(&a, &id).ok().flatten();
                    let _ = tx.send(SideMsg::Pin(ch, pin));
                    ctx.request_repaint();
                });
            }
        }
    }

    /// The pinned message to show above a tab's chat (where you moderate).
    pub(super) fn pin_of(&self, group: &str) -> Option<Pin> {
        self.pins
            .get(&self.twitch_of(group))
            .and_then(|(p, _)| p.clone())
    }

    pub(super) fn on_eventsub(&mut self, e: EsEvent) {
        match e {
            EsEvent::Held(ch, h) => {
                for g in self.groups_showing(&ch) {
                    self.push_to(&g, ChatLine::AutoMod(Box::new(h.clone())));
                }
            }
            EsEvent::HeldDone(ch, id, status, by) => {
                let note = if by.is_empty() {
                    status
                } else {
                    format!("{status} by {by}")
                };
                for g in self.groups_showing(&ch) {
                    if let Some(t) = self.chats.iter_mut().find(|t| t.login == g) {
                        for l in t.lines.iter_mut() {
                            if let ChatLine::AutoMod(h) = l {
                                if h.msg_id == id {
                                    h.status = note.clone();
                                }
                            }
                        }
                    }
                }
            }
            EsEvent::UnbanRequest(ch, r) => {
                let line = format!(
                    "New unban request from {}: \"{}\" (/unbanrequests to answer it)",
                    r.user_name, r.text
                );
                for g in self.groups_showing(&ch) {
                    self.push_to(&g, ChatLine::System(line.clone()));
                }
                if let Some(w) = self.unban_win.as_mut().filter(|w| w.channel == ch) {
                    if !w.items.iter().any(|i| i.id == r.id) {
                        w.items.push(r);
                    }
                }
            }
            EsEvent::UnbanResolved(ch, id, status, by) => {
                if let Some(w) = self.unban_win.as_mut().filter(|w| w.channel == ch) {
                    if let Some(i) = w.items.iter_mut().find(|i| i.id == id) {
                        i.status = status;
                        i.moderator = by;
                    }
                }
            }
            EsEvent::Mod(ch, entry) => {
                let list = self
                    .modlogs
                    .entry(ch.clone())
                    .or_insert_with(|| crate::modlog::load(&ch));
                list.push(entry.clone());
                crate::modlog::append(&ch, &entry, list);
                crate::modlog::trim(list);
            }
            EsEvent::Notice(ch, text) => {
                if self.es_noticed.insert(ch.clone()) {
                    for g in self.groups_showing(&ch) {
                        self.push_to(&g, ChatLine::System(text.clone()));
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------ unban requests

    pub(super) fn open_unban_requests(&mut self, group: &str) {
        let ch = self.twitch_of(group);
        self.unban_win = Some(UnbanWindow {
            channel: ch.clone(),
            items: Vec::new(),
            loading: true,
            error: String::new(),
            notes: HashMap::new(),
        });
        self.load_unban_requests(&ch);
    }

    fn load_unban_requests(&mut self, ch: &str) {
        let (Some(a), Some(id)) = (self.mod_auth(), self.rooms.get(ch).cloned()) else {
            if let Some(w) = self.unban_win.as_mut() {
                w.loading = false;
                w.error = "Sign in to Twitch, and open this channel's chat first.".into();
            }
            return;
        };
        let (tx, ctx, ch) = (self.side_tx.clone(), self.ctx.clone(), ch.to_string());
        std::thread::spawn(move || {
            let r = crate::moderation::unban_requests(&a, &id);
            let _ = tx.send(SideMsg::UnbanList(ch, r));
            ctx.request_repaint();
        });
    }

    pub(super) fn unban_list_loaded(&mut self, ch: &str, r: Result<Vec<UnbanRequest>, String>) {
        if let Some(w) = self.unban_win.as_mut().filter(|w| w.channel == ch) {
            w.loading = false;
            match r {
                Ok(items) => {
                    w.items = items;
                    w.error.clear();
                }
                Err(e) => w.error = e,
            }
        }
    }

    fn resolve_unban(&mut self, ch: &str, id: &str, approve: bool, note: String) {
        let (Some(a), Some(room)) = (self.mod_auth(), self.rooms.get(ch).cloned()) else {
            return;
        };
        let (tx, ctx) = (self.side_tx.clone(), self.ctx.clone());
        let (ch, id) = (ch.to_string(), id.to_string());
        std::thread::spawn(move || {
            let r = crate::moderation::resolve_unban(&a, &room, &id, approve, &note);
            let status = if approve { "approved" } else { "denied" };
            let _ = tx.send(SideMsg::UnbanDone(ch, id, status.into(), r));
            ctx.request_repaint();
        });
    }

    pub(super) fn unban_done(&mut self, ch: &str, id: &str, status: &str, r: Result<(), String>) {
        if let Some(w) = self.unban_win.as_mut().filter(|w| w.channel == ch) {
            match r {
                Ok(()) => {
                    if let Some(i) = w.items.iter_mut().find(|i| i.id == id) {
                        i.status = status.to_string();
                        i.moderator = "you".into();
                    }
                }
                Err(e) => w.error = e,
            }
        }
    }

    /// The unban requests window: pending requests with Approve / Deny and a note.
    pub(super) fn unban_window(&mut self, ctx: &egui::Context) {
        let Some(w) = self.unban_win.as_mut() else {
            return;
        };
        let mut open = true;
        let mut reload = false;
        let mut answer: Option<(String, bool, String)> = None;
        egui::Window::new(format!("Unban requests: {}", w.channel))
            .id(egui::Id::unique("unban-requests"))
            .open(&mut open)
            .default_size([460.0, 440.0])
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Reload").clicked() {
                        reload = true;
                    }
                    if w.loading {
                        ui.spinner();
                    }
                    let n = w.items.iter().filter(|i| i.status == "pending").count();
                    ui.weak(format!("{n} pending"));
                });
                if !w.error.is_empty() {
                    ui.colored_label(Color32::from_rgb(0xe8, 0x48, 0x48), &w.error);
                }
                if !w.loading && w.items.is_empty() {
                    ui.add_space(8.0);
                    ui.weak("No pending unban requests.");
                }
                egui::ScrollArea::vertical()
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for r in &w.items {
                            egui::Frame::group(ui.style()).show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.strong(&r.user_name);
                                    ui.weak(local_time(&r.created_at));
                                });
                                ui.label(&r.text);
                                if r.status == "pending" {
                                    let note = w.notes.entry(r.id.clone()).or_default();
                                    ui.add(
                                        egui::TextEdit::singleline(note)
                                            .hint_text("A note to them (optional)")
                                            .char_limit(500)
                                            .desired_width(f32::INFINITY),
                                    );
                                    ui.horizontal(|ui| {
                                        if ui.button("Approve (unban)").clicked() {
                                            answer = Some((r.id.clone(), true, note.clone()));
                                        }
                                        if ui.button("Deny").clicked() {
                                            answer = Some((r.id.clone(), false, note.clone()));
                                        }
                                    });
                                } else {
                                    let by = if r.moderator.is_empty() {
                                        String::new()
                                    } else {
                                        format!(" by {}", r.moderator)
                                    };
                                    ui.weak(format!("{}{by}", r.status));
                                }
                            });
                            ui.add_space(4.0);
                        }
                    });
            });
        let ch = w.channel.clone();
        if !open {
            self.unban_win = None;
            return;
        }
        if reload {
            if let Some(w) = self.unban_win.as_mut() {
                w.loading = true;
            }
            self.load_unban_requests(&ch);
        }
        if let Some((id, approve, note)) = answer {
            self.resolve_unban(&ch, &id, approve, note);
        }
    }

    // ------------------------------------------------------------ mod log

    pub(super) fn open_mod_log(&mut self, group: &str, filter: String) {
        let ch = self.twitch_of(group);
        self.modlogs
            .entry(ch.clone())
            .or_insert_with(|| crate::modlog::load(&ch));
        self.modlog_win = Some(ModLogWindow {
            channel: ch,
            filter,
        });
    }

    /// A user's mod log entries in a channel, newest first (for the usercard): by their
    /// Twitch id, or by name for entries without one.
    pub(super) fn mod_log_about(
        &mut self,
        channel: &str,
        user: &str,
        user_id: &str,
    ) -> Vec<crate::modlog::Entry> {
        self.modlogs
            .entry(channel.to_string())
            .or_insert_with(|| crate::modlog::load(channel))
            .iter()
            .rev()
            .filter(|e| {
                if !user_id.is_empty() && !e.target_id.is_empty() {
                    e.target_id == user_id
                } else {
                    e.target.eq_ignore_ascii_case(user)
                }
            })
            .take(20)
            .cloned()
            .collect()
    }

    /// The mod log window (/modlogs): newest first, filtered by user, moderator or action.
    pub(super) fn mod_log_window(&mut self, ctx: &egui::Context) {
        let Some(w) = self.modlog_win.as_mut() else {
            return;
        };
        let empty = Vec::new();
        let list = self.modlogs.get(&w.channel).unwrap_or(&empty);
        let mut open = true;
        egui::Window::new(format!("Mod log: {}", w.channel))
            .id(egui::Id::unique("mod-log"))
            .open(&mut open)
            .default_size([520.0, 460.0])
            .collapsible(false)
            .show(ctx, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut w.filter)
                        .hint_text("Filter by user, moderator or action")
                        .desired_width(f32::INFINITY),
                );
                let q = w.filter.trim().to_lowercase();
                let shown: Vec<&crate::modlog::Entry> = list
                    .iter()
                    .rev()
                    .filter(|e| {
                        q.is_empty()
                            || e.target.to_lowercase().contains(&q)
                            || e.moderator.to_lowercase().contains(&q)
                            || e.action.to_lowercase().contains(&q)
                            || e.detail.to_lowercase().contains(&q)
                    })
                    .take(2000)
                    .collect();
                ui.weak(format!(
                    "{} of {} actions. Yaperino records them while it runs; Twitch doesn't share older ones.",
                    shown.len(),
                    list.len()
                ));
                ui.separator();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if shown.is_empty() {
                            ui.weak("Nothing yet.");
                        }
                        for e in shown {
                            ui.horizontal_wrapped(|ui| {
                                let when = chrono::DateTime::from_timestamp(e.time, 0)
                                    .map(|t| {
                                        t.with_timezone(&chrono::Local)
                                            .format("%b %-d %H:%M")
                                            .to_string()
                                    })
                                    .unwrap_or_default();
                                ui.weak(when);
                                ui.label(e.describe());
                            });
                        }
                    });
            });
        if !open {
            self.modlog_win = None;
        }
    }

    // ------------------------------------------------------------ screenshot samples

    /// Screenshot mode: sample held messages and a pin (LIVE_STREAMS_SHOT_AUTOMOD, with
    /// _MOD), the unban requests window (_UNBAN) and the mod log (_MODLOGS).
    fn shot_mod_tools(&mut self) {
        let Some(first) = self.chats.first().map(|t| t.login.clone()) else {
            return;
        };
        if !self.es_noticed.insert(String::new()) {
            return;
        }
        let ch = self.twitch_of(&first);
        let env = |k: &str| std::env::var(k).is_ok();
        if env("LIVE_STREAMS_SHOT_AUTOMOD") {
            let held = |user: &str, text: &str, reason: &str, status: &str| {
                ChatLine::AutoMod(Box::new(crate::chat::HeldMessage {
                    time: chrono::Local::now().format("%H:%M").to_string(),
                    msg_id: format!("sample-{user}"),
                    user_id: "1".into(),
                    user: user.into(),
                    text: text.into(),
                    reason: reason.into(),
                    status: status.into(),
                }))
            };
            self.push_to(
                &first,
                held(
                    "AnotherViewer",
                    "this is a rude sample message",
                    "aggressive, level 2",
                    "",
                ),
            );
            self.push_to(
                &first,
                held(
                    "spammer42",
                    "buy followers at example dot com",
                    "blocked link",
                    "denied by ModOne",
                ),
            );
            self.pins.insert(
                ch.clone(),
                (
                    Some(Pin {
                        message_id: "sample".into(),
                        sender: "ModOne".into(),
                        text: "Welcome! Be nice, no spoilers, have fun.".into(),
                        pinned_by: "ModOne".into(),
                        ends_at: String::new(),
                    }),
                    Instant::now(),
                ),
            );
        }
        if env("LIVE_STREAMS_SHOT_UNBAN") {
            let req = |id: &str, user: &str, text: &str, status: &str| UnbanRequest {
                id: id.into(),
                user_login: user.to_lowercase(),
                user_name: user.into(),
                text: text.into(),
                created_at: "2026-11-17T18:30:00Z".into(),
                status: status.into(),
                moderator: if status == "pending" {
                    String::new()
                } else {
                    "ModOne".into()
                },
                ..Default::default()
            };
            self.unban_win = Some(UnbanWindow {
                channel: ch.clone(),
                items: vec![
                    req(
                        "1",
                        "spammer42",
                        "I'm sorry, it won't happen again.",
                        "pending",
                    ),
                    req(
                        "2",
                        "AnotherViewer",
                        "It was a joke, please unban me.",
                        "denied",
                    ),
                ],
                loading: false,
                error: String::new(),
                notes: HashMap::new(),
            });
        }
        if env("LIVE_STREAMS_SHOT_MODLOGS") {
            self.modlogs.insert(ch.clone(), crate::modlog::samples());
            self.modlog_win = Some(ModLogWindow {
                channel: ch,
                filter: String::new(),
            });
        }
    }
}

/// The pinned message above the chat (Twitch's pin); true when Unpin was clicked.
pub fn pin_bar(ui: &mut egui::Ui, r: Rect, pin: &Pin) -> bool {
    let c = crate::style::theme::c();
    ui.painter().rect_filled(r, 0.0, c.input_bg);
    ui.painter().line_segment(
        [r.left_bottom(), r.right_bottom()],
        Stroke::new(1.0, c.separator),
    );
    let inner = r.shrink2(Vec2::new(8.0, 2.0));
    let button = Rect::from_min_max(Pos2::new(inner.right() - 54.0, inner.top()), inner.max);
    let mut b = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(button)
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
    );
    let until = if pin.ends_at.is_empty() {
        "until the stream ends".to_string()
    } else {
        chrono::DateTime::parse_from_rfc3339(&pin.ends_at)
            .map(|t| format!("until {}", t.with_timezone(&chrono::Local).format("%H:%M")))
            .unwrap_or_default()
    };
    let unpin = b
        .small_button("Unpin")
        .on_hover_text("Unpin this message")
        .clicked();
    let mut s = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(Rect::from_min_max(
                inner.min,
                Pos2::new(button.left() - 6.0, inner.bottom()),
            ))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    s.spacing_mut().item_spacing.x = 4.0;
    s.label(egui::RichText::new("📌").color(c.accent));
    s.label(egui::RichText::new(format!("{}:", pin.sender)).strong());
    s.add(egui::Label::new(egui::RichText::new(&pin.text).color(c.text)).truncate())
        .on_hover_text(format!("Pinned by {} {until}", pin.pinned_by));
    unpin
}
