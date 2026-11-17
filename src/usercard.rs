//! Chatterino's usercard (Moltorino's user popup): click a name in chat. Shows the Twitch
//! profile (picture, account age, followers), when they followed this channel (Twitch only
//! tells moderators), their earlier names (`namehistory`), their messages here (this session
//! and the chat logs), and mod actions in channels you moderate. A child of `app`, so it
//! works on the window's own state.

use super::*;
use std::path::Path;
use std::sync::mpsc::Receiver;

/// What a button on the card asks the chat window to do.
pub enum CardAction {
    Moderate(crate::moderation::ModAction),
    /// Put "@name " into the message box.
    Mention(String),
    /// Moltorino /hide.
    Hide(String),
    /// Open the user's own channel.
    OpenChannel(String),
}

pub struct UserCard {
    pub name: String,
    pub user_id: String,
    pub platform: Platform,
    /// The tab (chat) the name was clicked in.
    pub group: String,
    /// That chat's Twitch channel (login), if it has one.
    pub channel: Option<String>,
    profile: Option<Result<Profile, String>>,
    rx: Option<Receiver<Result<Profile, String>>>,
    /// Their lines from the chat logs (newest last), read when the card opens.
    logs: Vec<String>,
    /// Their earlier Twitch names (newest first), once loaded.
    names: Option<Result<Vec<crate::namehistory::OldName>, String>>,
    names_rx: Option<Receiver<Result<Vec<crate::namehistory::OldName>, String>>>,
}

/// A Twitch profile, from Twitch's API (signed in) or api.ivr.fi.
#[derive(Clone, Default, Debug)]
pub struct Profile {
    pub login: String,
    pub display: String,
    pub id: String,
    pub description: String,
    pub avatar: String,
    /// Account creation, Unix seconds.
    pub created: Option<i64>,
    pub followers: Option<u64>,
    /// When they followed this channel: Some(Ok(None)) = not following, Some(Err) = Twitch
    /// doesn't say (only moderators may see it), None = not asked.
    pub followed: Option<Result<Option<i64>, String>>,
}

impl LiveApp {
    /// Opens the usercard for a clicked name and starts loading the profile.
    pub(super) fn open_usercard(&mut self, group: &str, user: crate::style::UserRef) {
        let channel = self
            .chats
            .iter()
            .find(|t| t.login == group)
            .and_then(|t| t.twitch.clone());
        let mut card = UserCard {
            name: user.name.clone(),
            user_id: user.user_id.clone(),
            platform: user.platform,
            group: group.to_string(),
            channel: channel.clone(),
            profile: None,
            rx: None,
            logs: Vec::new(),
            names: None,
            names_rx: None,
        };
        if let Some(ch) = &channel {
            card.logs = read_logs(&self.log_dir(), ch, &user.name, 14, 200);
        }
        if self.demo {
            card.profile = Some(Ok(sample_profile(&user.name)));
            card.names = Some(Ok(crate::namehistory::sample(&user.name)));
        } else if user.platform == Platform::Twitch {
            start_names(&mut card, &user.user_id, &self.ctx);
            let (tx, rx) = std::sync::mpsc::channel();
            card.rx = Some(rx);
            let login = user.name.to_lowercase();
            let channel_id = channel.as_ref().and_then(|c| self.rooms.get(c).cloned());
            let auth = self
                .account
                .as_ref()
                .map(|a| (a.client_id.clone(), a.token.clone()));
            let ctx = self.ctx.clone();
            std::thread::spawn(move || {
                let _ = tx.send(load_profile(&login, channel_id.as_deref(), auth));
                ctx.request_repaint();
            });
        }
        self.usercard = Some(card);
    }

    /// The chat logs' folder (Moderation > Logs, or the default).
    fn log_dir(&self) -> PathBuf {
        if self.settings.log_dir.trim().is_empty() {
            super::features::default_log_dir()
        } else {
            PathBuf::from(self.settings.log_dir.trim())
        }
    }

    /// Draws the open usercard; returns (the card's tab, what a button asked for).
    pub(super) fn usercard_window(&mut self, ctx: &egui::Context) -> Option<(String, CardAction)> {
        if self.shot.is_some()
            && self.usercard.is_none()
            && std::env::var("LIVE_STREAMS_SHOT_USERCARD").is_ok()
        {
            // screenshot test only: the card of a sample chatter
            if let Some(tab) = self.chats.first() {
                let group = tab.login.clone();
                self.open_usercard(
                    &group,
                    crate::style::UserRef {
                        name: "AnotherViewer".into(),
                        user_id: "12345".into(),
                        platform: Platform::Twitch,
                    },
                );
            }
        }
        let mut card = self.usercard.take()?;
        if let Some(r) = card.rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            // no user id from the chat line: the earlier names go by the profile's
            if let (Ok(p), None, None) = (&r, &card.names, &card.names_rx) {
                let id = p.id.clone();
                start_names(&mut card, &id, ctx);
            }
            card.profile = Some(r);
            card.rx = None;
        }
        if let Some(r) = card.names_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            card.names = Some(r);
            card.names_rx = None;
        }
        // their messages in this chat this session, and their colour and badges from them
        let mut session: Vec<(String, String)> = Vec::new();
        let mut color: Option<[u8; 3]> = None;
        let mut badges: Vec<(String, String)> = Vec::new();
        if let Some(t) = self.chats.iter().find(|t| t.login == card.group) {
            for l in &t.lines {
                if let ChatLine::Msg {
                    name,
                    time,
                    text,
                    color: c,
                    badges: b,
                    ..
                } = l
                {
                    if name.eq_ignore_ascii_case(&card.name) {
                        session.push((time.clone(), text.clone()));
                        color = c.or(color);
                        badges = b.clone();
                    }
                }
            }
        }
        let moderates = card.platform == Platform::Twitch
            && !card.user_id.is_empty()
            && self.account.as_ref().is_some_and(|a| a.can_moderate())
            && card
                .channel
                .as_ref()
                .is_some_and(|c| self.mod_in.contains(c))
            || (self.shot.is_some() && std::env::var("LIVE_STREAMS_SHOT_MOD").is_ok());
        // what moderators did to them here (the mod log Yaperino keeps)
        let mod_log = match (card.channel.clone(), moderates) {
            (Some(c), true) => self.mod_log_about(&c, &card.name, &card.user_id),
            _ => Vec::new(),
        };
        let timeouts: Vec<(String, u64)> = self
            .settings
            .timeout_buttons
            .iter()
            .filter_map(|t| {
                crate::moderation::parse_duration(t)
                    .map(|s| (crate::moderation::duration_text(s), s))
            })
            .collect();
        let logging = self.settings.logging;
        let p = crate::style::theme::c();
        let user_color = crate::style::name_color(color, &card.name);
        let profile = card.profile.as_ref().and_then(|p| p.as_ref().ok()).cloned();
        let shown_name = profile
            .as_ref()
            .map(|p| p.display.clone())
            .filter(|d| !d.is_empty())
            .unwrap_or_else(|| card.name.clone());
        let mut open = true;
        let mut action: Option<CardAction> = None;
        let mut window = egui::Window::new("usercard")
            .id(egui::Id::unique("usercard"))
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .frame(
                egui::Frame::window(&ctx.global_style())
                    .inner_margin(0)
                    .corner_radius(14)
                    .fill(p.split_bg),
            )
            .default_width(CARD_W);
        if self.shot.is_some() {
            window = window.anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO);
        }
        window.show(ctx, |ui| {
            ui.set_width(CARD_W);
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            // ---- banner in the user's colour, the picture over its edge, a close button
            let (banner, _) = ui.allocate_exact_size(Vec2::new(CARD_W, 70.0), Sense::hover());
            // the banner, rounded at the top like the card
            ui.painter().rect_filled(
                banner,
                egui::CornerRadius {
                    nw: 14,
                    ne: 14,
                    sw: 0,
                    se: 0,
                },
                user_color.gamma_multiply(0.8),
            );
            let close = egui::Rect::from_center_size(
                banner.right_top() + Vec2::new(-20.0, 20.0),
                Vec2::splat(26.0),
            );
            let close_r = ui.interact(close, ui.scope_id().with("usercard-close"), Sense::click());
            ui.painter().circle_filled(
                close.center(),
                13.0,
                Color32::from_black_alpha(if close_r.hovered() { 150 } else { 90 }),
            );
            let x = Stroke::new(1.6, Color32::WHITE);
            let c = close.center();
            ui.painter()
                .line_segment([c + Vec2::new(-4.5, -4.5), c + Vec2::new(4.5, 4.5)], x);
            ui.painter()
                .line_segment([c + Vec2::new(-4.5, 4.5), c + Vec2::new(4.5, -4.5)], x);
            if close_r.clicked() {
                open = false;
            }
            let avatar = egui::Rect::from_center_size(
                egui::pos2(banner.left() + 54.0, banner.bottom() + 6.0),
                Vec2::splat(76.0),
            );
            ui.painter()
                .circle_filled(avatar.center(), 40.0, p.split_bg);
            match profile.as_ref().map(|p| p.avatar.clone()).filter(|a| !a.is_empty()) {
                Some(url) => egui::Image::new(url)
                    .corner_radius(38.0)
                    .paint_at(ui, avatar.shrink(1.0)),
                None => {
                    ui.painter()
                        .circle_filled(avatar.center(), 37.0, user_color.gamma_multiply(0.55));
                    let initial = card.name.chars().next().unwrap_or('?').to_uppercase().to_string();
                    ui.painter().text(
                        avatar.center(),
                        egui::Align2::CENTER_CENTER,
                        initial,
                        egui::FontId::proportional(32.0),
                        Color32::WHITE,
                    );
                }
            }
            // ---- body
            egui::Frame::new()
                .inner_margin(egui::Margin {
                    left: 18,
                    right: 18,
                    top: 0,
                    bottom: 16,
                })
                .show(ui, |ui| {
                    // name next to the picture
                    ui.horizontal(|ui| {
                        ui.add_space(98.0);
                        ui.vertical(|ui| {
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(&shown_name)
                                        .size(20.0)
                                        .strong()
                                        .color(p.strong_text),
                                );
                                crate::style::platform_badge(ui, card.platform, 18.0)
                                    .on_hover_text(card.platform.label());
                            });
                            let login = profile
                                .as_ref()
                                .map(|p| p.login.clone())
                                .filter(|l| !l.is_empty() && !l.eq_ignore_ascii_case(&shown_name));
                            if let Some(l) = login {
                                ui.label(egui::RichText::new(l).color(p.tab_text));
                            }
                        });
                    });
                    ui.add_space(8.0);
                    // roles from their badges here
                    let roles: Vec<(&str, Color32)> = badges
                        .iter()
                        .filter_map(|(set, _)| match set.as_str() {
                            "broadcaster" => Some(("Broadcaster", Color32::from_rgb(0xe9, 0x19, 0x16))),
                            "moderator" => Some(("Moderator", Color32::from_rgb(0x00, 0xad, 0x03))),
                            "vip" => Some(("VIP", Color32::from_rgb(0xe0, 0x05, 0xb9))),
                            "subscriber" | "founder" => Some(("Subscriber", Color32::from_rgb(0x91, 0x46, 0xff))),
                            "partner" => Some(("Verified", Color32::from_rgb(0x91, 0x46, 0xff))),
                            "staff" | "admin" => Some(("Twitch staff", Color32::from_rgb(0x20, 0x20, 0x20))),
                            _ => None,
                        })
                        .collect();
                    if !roles.is_empty() {
                        ui.horizontal_wrapped(|ui| {
                            for (role, col) in roles {
                                pill(ui, role, col, Color32::WHITE);
                            }
                        });
                    }
                    // facts
                    let chip_bg = p.shade(30);
                    match &card.profile {
                        Some(Ok(pr)) => {
                            ui.horizontal_wrapped(|ui| {
                                if let Some(c) = pr.created {
                                    pill(ui, &format!("Joined {}", short_date(c)), chip_bg, p.text)
                                        .on_hover_text(format!("Account created {}", date_and_age(c)));
                                }
                                if let Some(f) = pr.followers {
                                    pill(
                                        ui,
                                        &format!("{} followers", crate::model::format_viewers(f)),
                                        chip_bg,
                                        p.text,
                                    );
                                }
                                if let (Some(ch), Some(Ok(f))) = (&card.channel, &pr.followed) {
                                    match f {
                                        Some(at) => {
                                            pill(
                                                ui,
                                                &format!("Following since {}", short_date(*at)),
                                                p.accent.gamma_multiply(0.45),
                                                p.strong_text,
                                            )
                                            .on_hover_text(format!(
                                                "Following #{ch} since {}",
                                                date_and_age(*at)
                                            ));
                                        }
                                        None => {
                                            pill(ui, "Not following", chip_bg, p.tab_text);
                                        }
                                    }
                                }
                            });
                            if let Some(Err(why)) = &pr.followed {
                                ui.label(egui::RichText::new(why).small().color(p.tab_text));
                            }
                            if !pr.description.is_empty() {
                                ui.add_space(2.0);
                                ui.label(egui::RichText::new(&pr.description).italics().color(p.text));
                            }
                        }
                        Some(Err(e)) => {
                            ui.label(egui::RichText::new(format!("Profile not loaded: {e}")).color(p.tab_text));
                        }
                        None if card.platform == Platform::Twitch => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(egui::RichText::new("Loading the profile...").color(p.tab_text));
                            });
                        }
                        None => {}
                    }
                    // their earlier Twitch names (logs.ivr.fi / logs.zonian.dev)
                    if let Some(Ok(names)) = &card.names {
                        let login = profile.as_ref().map(|p| p.login.clone()).unwrap_or_default();
                        let earlier = crate::namehistory::previous(names, &[&card.name, &login]);
                        if !earlier.is_empty() {
                            ui.add_space(2.0);
                            ui.label(egui::RichText::new("PREVIOUS NAMES").small().strong().color(p.tab_text));
                            for n in earlier {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(&n.login).strong().color(p.text));
                                    let when = match (n.first, n.last) {
                                        (Some(a), Some(b)) => format!("{} – {}", short_date(a), short_date(b)),
                                        (Some(a), None) => format!("since {}", short_date(a)),
                                        (None, Some(b)) => format!("until {}", short_date(b)),
                                        (None, None) => String::new(),
                                    };
                                    ui.label(egui::RichText::new(when).small().color(p.tab_text));
                                });
                            }
                        }
                    }
                    // actions
                    ui.add_space(6.0);
                    // one row: short labels, the details in the tooltips
                    ui.horizontal(|ui| {
                        ui.spacing_mut().button_padding = Vec2::new(8.0, 3.0);
                        if soft_button(ui, "@ Mention")
                            .on_hover_text("Put @name into the message box")
                            .clicked()
                        {
                            action = Some(CardAction::Mention(card.name.clone()));
                        }
                        if soft_button(ui, "Copy")
                            .on_hover_text("Copy the user name")
                            .clicked()
                        {
                            ui.ctx().copy_text(card.name.clone());
                        }
                        if soft_button(ui, "Hide")
                            .on_hover_text(
                                "Hide their messages here only; /unhide brings them back (Moltorino /hide)",
                            )
                            .clicked()
                        {
                            action = Some(CardAction::Hide(card.name.clone()));
                        }
                        if card.platform == Platform::Twitch {
                            if soft_button(ui, "Channel")
                                .on_hover_text("Open their own channel")
                                .clicked()
                            {
                                action = Some(CardAction::OpenChannel(card.name.to_lowercase()));
                            }
                            if let Some(ch) = &card.channel {
                                if soft_button(ui, "Logs ↗")
                                    .on_hover_text("All their messages in this channel on logs.ivr.fi")
                                    .clicked()
                                {
                                    crate::browser::open_url(&format!(
                                        "https://logs.ivr.fi/?channel={ch}&username={}",
                                        card.name.to_lowercase()
                                    ));
                                }
                            }
                        }
                    });
                    // moderation (Chatterino's usercard: timeouts, ban, unban)
                    if moderates {
                        ui.add_space(6.0);
                        egui::Frame::new()
                            .fill(p.shade(16))
                            .corner_radius(10)
                            .inner_margin(10)
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.label(
                                    egui::RichText::new("MODERATION")
                                        .small()
                                        .strong()
                                        .color(p.tab_text),
                                );
                                let act = |kind| crate::moderation::ModAction {
                                    user: card.name.clone(),
                                    user_id: card.user_id.clone(),
                                    kind,
                                };
                                // one row: the timeouts, then ban and unban
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 4.0;
                                    ui.spacing_mut().button_padding = Vec2::new(6.0, 3.0);
                                    for (label, secs) in &timeouts {
                                        if soft_button(ui, label)
                                            .on_hover_text(format!("Timeout for {label}"))
                                            .clicked()
                                        {
                                            action = Some(CardAction::Moderate(act(
                                                crate::moderation::ModKind::Timeout(*secs),
                                            )));
                                        }
                                    }
                                    ui.add_space(4.0);
                                    ui.separator();
                                    let ban = egui::Button::new(
                                        egui::RichText::new("Ban").strong().color(Color32::WHITE),
                                    )
                                    .fill(Color32::from_rgb(0xd6, 0x3b, 0x3b))
                                    .corner_radius(8)
                                    .min_size(Vec2::new(0.0, 28.0));
                                    if ui.add(ban).on_hover_text("Ban from the channel").clicked() {
                                        action = Some(CardAction::Moderate(act(
                                            crate::moderation::ModKind::Ban,
                                        )));
                                    }
                                    if soft_button(ui, "Unban")
                                        .on_hover_text("Lift a ban or timeout")
                                        .clicked()
                                    {
                                        action = Some(CardAction::Moderate(act(
                                            crate::moderation::ModKind::Unban,
                                        )));
                                    }
                                });
                            });
                    }
                    if !mod_log.is_empty() {
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("MOD LOG")
                                    .small()
                                    .strong()
                                    .color(p.tab_text),
                            );
                            pill(ui, &mod_log.len().to_string(), p.shade(30), p.text);
                        });
                        egui::Frame::new()
                            .fill(p.shade(10))
                            .corner_radius(10)
                            .inner_margin(egui::Margin::symmetric(10, 8))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                for e in &mod_log {
                                    let when = chrono::DateTime::from_timestamp(e.time, 0)
                                        .map(|t| {
                                            t.with_timezone(&chrono::Local)
                                                .format("%b %-d, %Y %H:%M")
                                                .to_string()
                                        })
                                        .unwrap_or_default();
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(
                                            egui::RichText::new(when)
                                                .small()
                                                .color(p.tab_text),
                                        );
                                        ui.label(egui::RichText::new(e.describe()).small());
                                    });
                                }
                            });
                    }
                    // their messages
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("MESSAGES HERE").small().strong().color(p.tab_text));
                        pill(ui, &session.len().to_string(), p.shade(30), p.text);
                    });
                    egui::Frame::new()
                        .fill(p.shade(10))
                        .corner_radius(10)
                        .inner_margin(egui::Margin::symmetric(10, 8))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            egui::ScrollArea::vertical()
                                .id_salt("usercard-history")
                                .max_height(220.0)
                                .stick_to_bottom(true)
                                .show(ui, |ui| {
                                    for l in &card.logs {
                                        ui.label(egui::RichText::new(l).small().color(p.tab_text));
                                    }
                                    if session.is_empty() && card.logs.is_empty() {
                                        ui.label(
                                            egui::RichText::new("Nothing from them here yet.")
                                                .color(p.tab_text),
                                        );
                                    }
                                    for (time, text) in &session {
                                        ui.horizontal_wrapped(|ui| {
                                            ui.label(
                                                egui::RichText::new(time)
                                                    .small()
                                                    .color(p.tab_text),
                                            );
                                            ui.label(egui::RichText::new(text).color(p.text));
                                        });
                                    }
                                });
                        });
                    if card.logs.is_empty() && !logging {
                        ui.label(
                            egui::RichText::new(
                                "Older messages: turn on chat logs in Settings > Moderation > Logs.",
                            )
                            .small()
                            .color(p.tab_text),
                        );
                    }
                });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            open = false;
        }
        if action.is_some() && !matches!(action, Some(CardAction::Moderate(_))) {
            open = false;
        }
        let group = card.group.clone();
        if open {
            self.usercard = Some(card);
        }
        action.map(|a| (group, a))
    }
}

/// The usercard's width.
const CARD_W: f32 = 440.0;

/// Starts loading a Twitch user's earlier names (by user id) for the card.
fn start_names(card: &mut UserCard, user_id: &str, ctx: &egui::Context) {
    if user_id.is_empty() {
        return;
    }
    let (tx, rx) = std::sync::mpsc::channel();
    card.names_rx = Some(rx);
    let (id, ctx) = (user_id.to_string(), ctx.clone());
    std::thread::spawn(move || {
        let _ = tx.send(crate::namehistory::load(&id));
        ctx.request_repaint();
    });
}

/// A small rounded label (role, fact, count).
fn pill(ui: &mut egui::Ui, text: &str, fill: Color32, color: Color32) -> egui::Response {
    egui::Frame::new()
        .fill(fill)
        .corner_radius(10)
        .inner_margin(egui::Margin::symmetric(9, 3))
        .show(ui, |ui| {
            // never broken inside: a pill that doesn't fit moves to the next line whole
            ui.add(
                egui::Label::new(egui::RichText::new(text).size(12.5).color(color))
                    .wrap_mode(egui::TextWrapMode::Extend),
            );
        })
        .response
}

/// A soft, rounded button (the card's actions).
fn soft_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let p = crate::style::theme::c();
    ui.add(
        egui::Button::new(egui::RichText::new(text).color(p.strong_text))
            .fill(p.shade(26))
            .stroke(Stroke::NONE)
            .corner_radius(8)
            .min_size(Vec2::new(0.0, 28.0)),
    )
}

/// "Nov 18, 2022" from Unix seconds (the pills; the age is in their tooltips).
fn short_date(unix: i64) -> String {
    chrono::DateTime::from_timestamp(unix, 0)
        .map(|d| d.format("%b %-d, %Y").to_string())
        .unwrap_or_default()
}

/// "May 1, 2016 (9 years)" from Unix seconds.
fn date_and_age(unix: i64) -> String {
    let date = chrono::DateTime::from_timestamp(unix, 0)
        .map(|d| d.format("%B %-d, %Y").to_string())
        .unwrap_or_default();
    let days = (crate::model::now_unix() - unix).max(0) / 86_400;
    let age = if days >= 365 {
        let y = days / 365;
        format!("{y} year{}", if y == 1 { "" } else { "s" })
    } else if days >= 30 {
        let m = days / 30;
        format!("{m} month{}", if m == 1 { "" } else { "s" })
    } else {
        format!("{days} day{}", if days == 1 { "" } else { "s" })
    };
    format!("{date} ({age})")
}

/// A sample profile for screenshot mode (no network).
fn sample_profile(name: &str) -> Profile {
    let now = crate::model::now_unix();
    Profile {
        login: name.to_lowercase(),
        display: name.to_string(),
        id: "12345".into(),
        description: "Sample chatter (screenshot mode)".into(),
        avatar: String::new(),
        created: Some(now - 4 * 365 * 86_400),
        followers: Some(1_284),
        followed: Some(Ok(Some(now - 400 * 86_400))),
    }
}

/// A user's lines from the chat logs of the last `days` days (`<dir>/<channel>/<channel>-
/// <date>.log`, see features::log_line), oldest first, at most `max`, with the date in front.
pub fn read_logs(dir: &Path, channel: &str, name: &str, days: i64, max: usize) -> Vec<String> {
    let folder = dir.join(channel);
    let today = chrono::Local::now().date_naive();
    let mut out = Vec::new();
    for back in (0..days).rev() {
        let day = today - chrono::Duration::days(back);
        let file = folder.join(format!("{channel}-{}.log", day.format("%Y-%m-%d")));
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        for line in text.lines() {
            if log_line_author(line).is_some_and(|a| a.eq_ignore_ascii_case(name)) {
                out.push(format!("{} {line}", day.format("%m-%d")));
            }
        }
    }
    let skip = out.len().saturating_sub(max);
    out.split_off(skip)
}

/// The author of a chat log line: "[12:34] name: text", "[12:34] [K] name: text" or
/// "[12:34] name text" (/me).
fn log_line_author(line: &str) -> Option<&str> {
    let rest = line.strip_prefix('[')?.split_once("] ")?.1;
    // the platform tag of Kick / YouTube / TikTok lines
    let rest = match rest.strip_prefix('[') {
        Some(r) => r.split_once("] ")?.1,
        None => rest,
    };
    let end = rest.find([':', ' '])?;
    let name = &rest[..end];
    (!name.is_empty()).then_some(name)
}

/// The Twitch profile: Twitch's API when signed in, else api.ivr.fi (no account needed).
fn load_profile(
    login: &str,
    channel_id: Option<&str>,
    auth: Option<(String, String)>,
) -> Result<Profile, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(10)))
        .http_status_as_error(false)
        .build()
        .into();
    let Some((client_id, token)) = auth else {
        return load_profile_ivr(&agent, login);
    };
    let get = |url: &str| -> Result<(u16, serde_json::Value), String> {
        let mut r = agent
            .get(url)
            .header("Authorization", &format!("Bearer {token}"))
            .header("Client-Id", &client_id)
            .call()
            .map_err(|e| e.to_string())?;
        let status = r.status().as_u16();
        let v = r
            .body_mut()
            .read_json::<serde_json::Value>()
            .unwrap_or(serde_json::Value::Null);
        Ok((status, v))
    };
    let (status, v) = get(&format!("https://api.twitch.tv/helix/users?login={login}"))?;
    if status != 200 {
        return load_profile_ivr(&agent, login);
    }
    let u = v
        .get("data")
        .and_then(|d| d.get(0))
        .ok_or("no such Twitch user")?;
    let s = |k: &str| u.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let mut p = Profile {
        login: s("login"),
        display: s("display_name"),
        id: s("id"),
        description: s("description"),
        avatar: s("profile_image_url"),
        created: crate::model::parse_iso8601(&s("created_at")),
        followers: None,
        followed: None,
    };
    // their followers (any sign-in may read the total)
    if let Ok((200, f)) = get(&format!(
        "https://api.twitch.tv/helix/channels/followers?broadcaster_id={}&first=1",
        p.id
    )) {
        p.followers = f.get("total").and_then(|t| t.as_u64());
    }
    // when they followed this channel: Twitch only tells its moderators
    if let Some(ch) = channel_id {
        p.followed = Some(
            match get(&format!(
                "https://api.twitch.tv/helix/channels/followers?broadcaster_id={ch}&user_id={}",
                p.id
            )) {
                Ok((200, f)) => Ok(f
                    .get("data")
                    .and_then(|d| d.get(0))
                    .and_then(|x| x.get("followed_at"))
                    .and_then(|x| x.as_str())
                    .and_then(crate::model::parse_iso8601)),
                Ok((401 | 403, _)) => Err(
                    "Following since: Twitch only shows this to the channel's moderators \
                 (sign in to Twitch again if you are one)"
                        .into(),
                ),
                Ok((code, _)) => Err(format!("Following since: Twitch answered {code}")),
                Err(e) => Err(format!("Following since: {e}")),
            },
        );
    }
    Ok(p)
}

fn load_profile_ivr(agent: &ureq::Agent, login: &str) -> Result<Profile, String> {
    let v: serde_json::Value = agent
        .get(&format!("{}{login}", crate::fetch::IVR_URL))
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_json()
        .map_err(|e| e.to_string())?;
    let u = v.get(0).ok_or("no such Twitch user")?;
    let s = |k: &str| u.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    Ok(Profile {
        login: s("login"),
        display: s("displayName"),
        id: s("id"),
        description: s("bio"),
        avatar: s("logo"),
        created: crate::model::parse_iso8601(&s("createdAt")),
        followers: u.get("followers").and_then(|f| f.as_u64()),
        followed: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_line_authors() {
        assert_eq!(
            log_line_author("[12:34] viewer_one: hello"),
            Some("viewer_one")
        );
        assert_eq!(
            log_line_author("[12:34:56] [K] kick_viewer: hi"),
            Some("kick_viewer")
        );
        assert_eq!(log_line_author("[12:34] someone waves"), Some("someone"));
        assert_eq!(log_line_author("not a log line"), None);
    }

    #[test]
    fn reads_a_users_lines_from_the_logs() {
        let dir =
            std::env::temp_dir().join(format!("yaperino-usercard-test-{}", std::process::id()));
        let folder = dir.join("chan");
        std::fs::create_dir_all(&folder).unwrap();
        let today = chrono::Local::now().date_naive();
        let file = folder.join(format!("chan-{}.log", today.format("%Y-%m-%d")));
        std::fs::write(
            &file,
            "[10:00] alice: first\n[10:01] bob: not me\n[10:02] [YT] Alice: second\n",
        )
        .unwrap();
        let lines = read_logs(&dir, "chan", "alice", 3, 10);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].ends_with("[10:00] alice: first"));
        assert!(lines[1].ends_with("[10:02] [YT] Alice: second"));
    }

    #[test]
    fn ages() {
        let now = crate::model::now_unix();
        assert!(date_and_age(now - 2 * 365 * 86_400 - 86_400).ends_with("(2 years)"));
        assert!(date_and_age(now - 45 * 86_400).ends_with("(1 month)"));
        assert!(date_and_age(now - 86_400).ends_with("(1 day)"));
    }
}
