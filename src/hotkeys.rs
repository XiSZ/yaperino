//! Hotkeys, as Chatterino's Hotkeys page: every action with its default keys
//! (HotkeyController.cpp), which the person can change; changes are saved as
//! action id -> keys (empty = no keys).

use egui::{Key, Modifiers};
use std::collections::HashMap;

/// (id, category, name, default keys)
pub const ACTIONS: [(&str, &str, &str, &str); 28] = [
    ("settings", "Window", "open settings", "Ctrl+P"),
    ("new_tab", "Window", "new tab", "Ctrl+Shift+T"),
    ("close_tab", "Window", "close tab", "Ctrl+Shift+W"),
    ("next_tab", "Window", "select next tab", "Ctrl+Tab"),
    (
        "prev_tab",
        "Window",
        "select previous tab",
        "Ctrl+Shift+Tab",
    ),
    ("tab_1", "Window", "select tab #1", "Ctrl+1"),
    ("tab_2", "Window", "select tab #2", "Ctrl+2"),
    ("tab_3", "Window", "select tab #3", "Ctrl+3"),
    ("tab_4", "Window", "select tab #4", "Ctrl+4"),
    ("tab_5", "Window", "select tab #5", "Ctrl+5"),
    ("tab_6", "Window", "select tab #6", "Ctrl+6"),
    ("tab_7", "Window", "select tab #7", "Ctrl+7"),
    ("tab_8", "Window", "select tab #8", "Ctrl+8"),
    ("last_tab", "Window", "select last tab", "Ctrl+9"),
    ("popup_tab", "Window", "pop out tab", "Ctrl+Shift+N"),
    ("toggle_tabs", "Window", "toggle tab visibility", "Ctrl+U"),
    ("reopen_split", "Window", "reopen closed split", "Ctrl+G"),
    // Chatterino's global search: every open chat at once
    ("search_all", "Window", "search all chats", "Ctrl+Shift+F"),
    (
        "live_window",
        "Window",
        "live streams window (Chatty)",
        "Alt+L",
    ),
    (
        "join_channel",
        "Split",
        "join a channel (new tab)",
        "Ctrl+J",
    ),
    (
        "leave_channel",
        "Split",
        "leave the channel (closes its chat everywhere)",
        "Ctrl+L",
    ),
    ("new_split", "Split", "new split", "Ctrl+T"),
    ("close_split", "Split", "close split", "Ctrl+W"),
    ("change_channel", "Split", "change channel", "Ctrl+R"),
    ("popup_split", "Split", "pop out split", "Ctrl+N"),
    ("search", "Split", "search messages", "Ctrl+F"),
    ("focus_left", "Split", "focus split left", "Alt+Left"),
    ("focus_right", "Split", "focus split right", "Alt+Right"),
];

/// Shortcuts in the message box that aren't changeable (shown in the table).
pub const FIXED: [(&str, &str); 5] = [
    ("emote popup", "Ctrl+E"),
    ("send message", "Enter"),
    ("previous / next sent message", "Up / Down"),
    ("complete emote or user", "Tab / Shift+Tab"),
    ("choose a suggestion", "Up / Down, Tab or Enter"),
];

pub fn default_of(id: &str) -> &'static str {
    ACTIONS
        .iter()
        .find(|a| a.0 == id)
        .map(|a| a.3)
        .unwrap_or("")
}

/// The keys of an action: the person's own, else the default.
pub fn keys_of<'a>(id: &str, overrides: &'a HashMap<String, String>) -> &'a str {
    match overrides.get(id) {
        Some(k) => k.as_str(),
        None => default_of(id),
    }
}

/// "Ctrl+Shift+T" -> modifiers and key.
pub fn parse(s: &str) -> Option<(Modifiers, Key)> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let mut m = Modifiers::NONE;
    let parts: Vec<&str> = s.split('+').map(str::trim).collect();
    let (key, mods) = parts.split_last()?;
    for p in mods {
        match p.to_lowercase().as_str() {
            "ctrl" | "control" | "cmd" => m.command = true,
            "shift" => m.shift = true,
            "alt" => m.alt = true,
            _ => return None,
        }
    }
    if m.command {
        m.ctrl = !cfg!(target_os = "macos");
        m.mac_cmd = cfg!(target_os = "macos");
    }
    Some((m, Key::from_name(key)?))
}

/// Modifiers and key -> "Ctrl+Shift+T".
pub fn format(m: Modifiers, key: Key) -> String {
    let mut s = String::new();
    if m.command || m.ctrl {
        s.push_str("Ctrl+");
    }
    if m.shift {
        s.push_str("Shift+");
    }
    if m.alt {
        s.push_str("Alt+");
    }
    s.push_str(key.name());
    s
}

/// The same modifiers (Ctrl, Shift, Alt), however Ctrl was reported.
fn same(a: Modifiers, b: Modifiers) -> bool {
    (a.command || a.ctrl) == (b.command || b.ctrl) && a.shift == b.shift && a.alt == b.alt
}

/// Was this action's shortcut pressed? Takes the key press out of the input, as
/// Chatterino's hotkeys do.
pub fn pressed(ctx: &egui::Context, id: &str, overrides: &HashMap<String, String>) -> bool {
    let Some((m, key)) = parse(keys_of(id, overrides)) else {
        return false;
    };
    ctx.input_mut(|i| {
        let pos = i.events.iter().position(|e| matches!(e, egui::Event::Key { key: k, pressed: true, modifiers, .. } if *k == key && same(*modifiers, m)));
        match pos {
            Some(p) => {
                i.events.remove(p);
                true
            }
            None => false,
        }
    })
}

/// While recording: the first key pressed with its modifiers (not a lone modifier).
pub fn recorded(ctx: &egui::Context) -> Option<(Modifiers, Key)> {
    ctx.input_mut(|i| {
        let pos = i
            .events
            .iter()
            .position(|e| matches!(e, egui::Event::Key { pressed: true, .. }))?;
        if let egui::Event::Key { key, modifiers, .. } = i.events.remove(pos) {
            Some((modifiers, key))
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_t_and_ctrl_w_are_recognised() {
        // as egui-winit reports Ctrl+T / Ctrl+W on Windows: ctrl and command both set
        let ctx = egui::Context::default();
        let press = |key| egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::CTRL | Modifiers::COMMAND,
        };
        let input = egui::RawInput {
            events: vec![press(Key::T), press(Key::W), press(Key::J), press(Key::L)],
            ..Default::default()
        };
        let o = HashMap::new();
        ctx.begin_pass(input);
        // Ctrl+Shift+T must not take plain Ctrl+T
        let got = (
            pressed(&ctx, "new_tab", &o),
            pressed(&ctx, "new_split", &o),
            pressed(&ctx, "close_split", &o),
        );
        assert_eq!(got, (false, true, true));
        assert!(pressed(&ctx, "join_channel", &o) && pressed(&ctx, "leave_channel", &o));
    }

    #[test]
    fn parses_and_formats() {
        let (m, k) = parse("Ctrl+Shift+T").unwrap();
        assert!(m.command && m.shift && !m.alt);
        assert_eq!(k, Key::T);
        assert_eq!(format(m, k), "Ctrl+Shift+T");
        assert_eq!(parse("Alt+Left").map(|(_, k)| k), Some(Key::ArrowLeft));
        assert!(parse("").is_none());
        assert!(parse("Hyper+X").is_none());
        let mut o = HashMap::new();
        o.insert("search".to_string(), "Ctrl+K".to_string());
        assert_eq!(keys_of("search", &o), "Ctrl+K");
        assert_eq!(keys_of("settings", &o), "Ctrl+P");
        for (_, _, _, d) in ACTIONS {
            assert!(parse(d).is_some(), "default {d} parses");
        }
    }
}
