//! Chatterino's search across all chats (Ctrl+Shift+F): one search box over every open
//! chat; the matches are grouped by tab, and clicking one goes to that tab with its search
//! (Ctrl+F) set to the same text. A child of `app`, so it works on the window's own state.

use super::*;

#[derive(Default)]
pub struct GlobalSearch {
    pub query: String,
    /// The search box has been given the keyboard once.
    focused: bool,
}

/// One match: the tab, and the message's time, author, colour, platform and text.
struct Hit {
    time: String,
    name: String,
    color: Option<[u8; 3]>,
    platform: Platform,
    text: String,
    /// The message itself (for its menu: Translate) and its translation, if it has one.
    line: ChatLine,
    note: Option<String>,
}

/// The matching messages of one chat, oldest first (case-insensitive, author or text).
fn matches(lines: &VecDeque<ChatLine>, query: &str) -> Vec<Hit> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| match l {
            ChatLine::Msg {
                time,
                name,
                color,
                platform,
                text,
                ..
            } if name.to_lowercase().contains(&q) || text.to_lowercase().contains(&q) => {
                let note = lines
                    .get(i + 1)
                    .filter(|n| super::translate::is_note(n))
                    .and_then(|n| match n {
                        ChatLine::System(t) => Some(t.clone()),
                        _ => None,
                    });
                Some(Hit {
                    time: time.clone(),
                    name: name.clone(),
                    color: *color,
                    platform: *platform,
                    text: text.clone(),
                    line: l.clone(),
                    note,
                })
            }
            _ => None,
        })
        .collect()
}

impl LiveApp {
    /// Draws the search window; returns (a clicked match's tab, the query).
    pub(super) fn global_search_window(&mut self, ctx: &egui::Context) -> Option<(String, String)> {
        if self.shot.is_some() && self.global_search.is_none() {
            // screenshot test only: the window with a query
            if let Ok(q) = std::env::var("LIVE_STREAMS_SHOT_SEARCH") {
                self.global_search = Some(GlobalSearch {
                    query: q,
                    focused: true,
                });
            }
        }
        let mut gs = self.global_search.take()?;
        let p = crate::style::theme::c();
        // the matches of every chat, grouped by tab (in tab order)
        let groups: Vec<(String, String, Vec<Hit>)> = self
            .chats
            .iter()
            .map(|t| {
                (
                    t.login.clone(),
                    t.display.clone(),
                    matches(&t.lines, &gs.query),
                )
            })
            .filter(|(_, _, hits)| !hits.is_empty())
            .collect();
        let total: usize = groups.iter().map(|(_, _, h)| h.len()).sum();
        let mut open = true;
        let mut picked: Option<(String, String)> = None;
        let mut translate: Option<(String, ChatLine)> = None;
        let mut window = egui::Window::new("Search all chats")
            .id(egui::Id::unique("global-search"))
            .open(&mut open)
            .collapsible(false)
            .default_size([520.0, 440.0]);
        if self.shot.is_some() {
            window = window.anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO);
        }
        window.show(ctx, |ui| {
            let r = ui.add(
                egui::TextEdit::singleline(&mut gs.query)
                    .hint_text("Search every open chat (name or text)...")
                    .desired_width(f32::INFINITY),
            );
            if !gs.focused {
                r.request_focus();
                gs.focused = true;
            }
            ui.add_space(2.0);
            if gs.query.trim().is_empty() {
                ui.label(
                    egui::RichText::new("Type to search; click a match to go there.")
                        .color(p.tab_text),
                );
                return;
            }
            ui.label(
                egui::RichText::new(format!(
                    "{total} match{} in {} chat{}",
                    if total == 1 { "" } else { "es" },
                    groups.len(),
                    if groups.len() == 1 { "" } else { "s" }
                ))
                .small()
                .color(p.tab_text),
            );
            egui::ScrollArea::vertical()
                .id_salt("global-search-results")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (key, display, hits) in &groups {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(display).strong().color(p.strong_text));
                            ui.label(
                                egui::RichText::new(format!("{}", hits.len()))
                                    .small()
                                    .color(p.tab_text),
                            );
                        });
                        // newest first
                        for h in hits.iter().rev().take(200) {
                            let row = ui
                                .horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 4.0;
                                    ui.label(
                                        egui::RichText::new(&h.time).small().color(p.tab_text),
                                    );
                                    crate::style::platform_badge(ui, h.platform, 15.0);
                                    ui.label(
                                        egui::RichText::new(format!("{}:", h.name))
                                            .strong()
                                            .color(crate::style::name_color(h.color, &h.name)),
                                    );
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&h.text).color(p.text),
                                        )
                                        .truncate(),
                                    );
                                })
                                .response;
                            let click = ui.interact(row.rect, row.id.with("hit"), Sense::click());
                            if click.hovered() {
                                ui.painter().rect_filled(
                                    row.rect,
                                    2.0,
                                    p.shade(20).gamma_multiply(0.5),
                                );
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            let click = click.on_hover_text(format!(
                                "Go to {display} (right-click: Translate, Copy)"
                            ));
                            if click.clicked() {
                                picked = Some((key.clone(), gs.query.trim().to_string()));
                            }
                            // the message menu's Translate and Copy, here too
                            click.context_menu(|ui| {
                                if ui.button("Translate").clicked() {
                                    translate = Some((key.clone(), h.line.clone()));
                                    ui.close();
                                }
                                if ui.button("Copy message").clicked() {
                                    ui.ctx().copy_text(h.text.clone());
                                    ui.close();
                                }
                                if ui.button(format!("Go to {display}")).clicked() {
                                    picked = Some((key.clone(), gs.query.trim().to_string()));
                                    ui.close();
                                }
                            });
                            if let Some(n) = &h.note {
                                ui.label(egui::RichText::new(n).small().color(p.system));
                            }
                        }
                    }
                });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            open = false;
        }
        if let Some((group, line)) = translate {
            self.translate_line(&group, &line, true);
        }
        if open && picked.is_none() {
            self.global_search = Some(gs);
        }
        picked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(name: &str, text: &str) -> ChatLine {
        ChatLine::Msg {
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
        }
    }

    #[test]
    fn finds_by_name_or_text_ignoring_case() {
        let lines: VecDeque<ChatLine> = [
            msg("Alice", "hello"),
            msg("bob", "Alice is here"),
            msg("carol", "nothing"),
            ChatLine::System("alice joined".into()),
        ]
        .into_iter()
        .collect();
        let hits = matches(&lines, "  ALICE ");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].name, "Alice");
        assert_eq!(hits[1].name, "bob");
        assert!(matches(&lines, "").is_empty());
    }

    #[test]
    fn a_match_brings_its_translation() {
        let lines: VecDeque<ChatLine> = [
            msg("anna", "Hallo zusammen"),
            ChatLine::System("Translated from German: Hello everyone".into()),
            msg("anna", "noch was"),
            ChatLine::System("anna joined".into()),
        ]
        .into_iter()
        .collect();
        let hits = matches(&lines, "anna");
        assert_eq!(hits.len(), 2);
        assert_eq!(
            hits[0].note.as_deref(),
            Some("Translated from German: Hello everyone")
        );
        // an ordinary system line under a message isn't its translation
        assert_eq!(hits[1].note, None);
    }
}
