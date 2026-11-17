//! What the chatterino7-style Settings pages switch on: highlights (regex, case, flash,
//! sound, badges, blacklist), ignores (block / replace), filters, chat logs, live
//! notification sounds, always on top, start with Windows and plugins.
//! A child of `app`, so it works on the window's own state.

use super::*;
use crate::rules::{self, Hit, Ignore, Matchers, Rule};

/// The system's notification sound (Chatterino's highlight / live sound).
pub fn play_sound() {
    #[cfg(windows)]
    unsafe {
        // MB_ICONASTERISK: the "Asterisk" / notification sound of the Windows sound scheme
        windows_sys::Win32::System::Diagnostics::Debug::MessageBeep(0x40);
    }
    // macOS: a built-in system sound, played by the system's own player
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("afplay")
            .arg("/System/Library/Sounds/Glass.aiff")
            .spawn();
    }
    // Linux: the desktop's "new message" sound (freedesktop sound theme); quietly nothing
    // when neither player is installed
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let played = std::process::Command::new("canberra-gtk-play")
            .args(["-i", "message-new-instant"])
            .spawn()
            .is_ok();
        if !played {
            let _ = std::process::Command::new("paplay")
                .arg("/usr/share/sounds/freedesktop/stereo/message.oga")
                .spawn();
        }
    }
}

/// Where chat logs go when no folder is set: %APPDATA%\Yaperino\Logs on Windows.
pub fn default_log_dir() -> PathBuf {
    crate::paths::app_dir_or_here().join("Logs")
}

/// Total size of the files in a folder (for "Your logs currently take up ...").
pub fn folder_size(dir: &std::path::Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| {
            let p = e.path();
            if p.is_dir() {
                folder_size(&p)
            } else {
                e.metadata().map(|m| m.len()).unwrap_or(0)
            }
        })
        .sum()
}

pub fn human_size(n: u64) -> String {
    match n {
        0..=1023 => format!("{n} bytes"),
        1024..=1_048_575 => format!("{:.1} KB", n as f64 / 1024.0),
        _ => format!("{:.1} MB", n as f64 / 1_048_576.0),
    }
}

/// A file or folder name made safe (channel keys like kick:name or youtube:@x).
fn safe_name(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' || c == '@' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

// ------------------------------------------------------------- notifications

/// The app id Windows shows notifications under (with the logo and the name Yaperino).
pub const APP_USER_MODEL_ID: &str = "Yaperino";

/// Registers Yaperino's name and logo for Windows notifications (HKCU, no admin) and
/// gives this program that id, so notifications and the taskbar say "Yaperino".
pub fn register_app_id() {
    #[cfg(windows)]
    {
        let Some(dir) =
            std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join(crate::APP_ID))
        else {
            return;
        };
        let png = dir.join("yaperino-notify.png");
        if !png.is_file() {
            let _ = std::fs::create_dir_all(&dir);
            if let Some(img) = image::RgbaImage::from_raw(128, 128, crate::logo::rgba(128)) {
                let _ = img.save(&png);
            }
        }
        let key = format!(r"HKCU\Software\Classes\AppUserModelId\{APP_USER_MODEL_ID}");
        for (name, value) in [
            ("DisplayName", crate::APP_ID.to_string()),
            ("IconUri", png.display().to_string()),
        ] {
            let mut cmd = std::process::Command::new("reg");
            cmd.args(["add", &key, "/v", name, "/t", "REG_SZ", "/d", &value, "/f"]);
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
            let _ = cmd.status();
        }
        let wide: Vec<u16> = APP_USER_MODEL_ID
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(wide.as_ptr());
        }
    }
}

/// A system notification; `clicked` runs when it is clicked (Windows and Linux; macOS shows
/// it without a click action).
pub fn show_toast(
    title: &str,
    text: &str,
    clicked: impl Fn() + Send + Sync + 'static,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        use tauri_winrt_notification::Toast;
        let clicked = std::sync::Arc::new(clicked);
        let make = |id: &str| {
            let c = clicked.clone();
            Toast::new(id)
                .title(title)
                .text1(text)
                .on_activated(move |_| {
                    c();
                    Ok(())
                })
        };
        // Yaperino's own id; Windows' PowerShell id when this Windows doesn't take it
        make(APP_USER_MODEL_ID)
            .show()
            .or_else(|_| make(Toast::POWERSHELL_APP_ID).show())
            .map_err(|e| e.to_string())
    }
    // macOS: Notification Center (the app's bundle id, so it shows as Yaperino)
    #[cfg(target_os = "macos")]
    {
        let _ = clicked;
        let _ = notify_rust::set_application(MAC_BUNDLE_ID);
        notify_rust::Notification::new()
            .summary(title)
            .body(text)
            .show()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    // Linux: the desktop's notifications (D-Bus); clicking one runs `clicked`
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let handle = notify_rust::Notification::new()
            .appname(crate::APP_ID)
            .summary(title)
            .body(text)
            .action("default", "Open chat")
            .show()
            .map_err(|e| e.to_string())?;
        // waiting for the click blocks, so it gets its own thread
        std::thread::spawn(move || {
            handle.wait_for_action(|action| {
                if action == "default" {
                    clicked();
                }
            })
        });
        Ok(())
    }
}

/// The macOS app's bundle id (Info.plist in .github/workflows/build.yml).
#[cfg(target_os = "macos")]
const MAC_BUNDLE_ID: &str = "io.github.xisz.yaperino";

// ------------------------------------------------------------- image uploader

/// External tools > Image Uploader (Chatterino's): where a pasted image goes.
#[derive(Clone)]
pub struct Uploader {
    pub url: String,
    pub field: String,
    /// "Name: value; Name2: value2"
    pub headers: String,
    /// "{link}" or a path into the JSON answer like "{data.link}"; empty = the whole answer.
    pub link: String,
    pub delete: String,
}

/// Fills "{a.b}" in a pattern from a JSON answer (Chatterino's image link patterns).
pub fn fill_pattern(pattern: &str, answer: &serde_json::Value) -> String {
    let mut out = String::new();
    let mut rest = pattern;
    while let Some(i) = rest.find('{') {
        out.push_str(&rest[..i]);
        let Some(j) = rest[i..].find('}') else {
            out.push_str(&rest[i..]);
            return out;
        };
        let path = &rest[i + 1..i + j];
        let mut v = answer;
        for part in path.split('.') {
            v = v.get(part).unwrap_or(&serde_json::Value::Null);
        }
        match v {
            serde_json::Value::String(s) => out.push_str(s),
            serde_json::Value::Null => {}
            other => out.push_str(&other.to_string()),
        }
        rest = &rest[i + j + 1..];
    }
    out.push_str(rest);
    out
}

/// Uploads a PNG; returns (image link, deletion link).
pub fn upload_image(cfg: &Uploader, png: &[u8]) -> Result<(String, String), String> {
    let boundary = format!("----yaperino{}", crate::model::now_unix());
    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{}\"; filename=\"image.png\"\r\nContent-Type: image/png\r\n\r\n", cfg.field.trim()).as_bytes());
    body.extend_from_slice(png);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(60)))
        .http_status_as_error(false)
        .user_agent("Yaperino")
        .build()
        .into();
    let mut req = agent.post(cfg.url.trim()).header(
        "Content-Type",
        &format!("multipart/form-data; boundary={boundary}"),
    );
    for h in cfg.headers.split(';') {
        if let Some((k, v)) = h.split_once(':') {
            if !k.trim().is_empty() {
                req = req.header(k.trim(), v.trim());
            }
        }
    }
    let mut r = req.send(&body[..]).map_err(|e| e.to_string())?;
    let status = r.status().as_u16();
    let text = r.body_mut().read_to_string().unwrap_or_default();
    if !(200..300).contains(&status) {
        return Err(format!("the image host answered {status}"));
    }
    let json: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
    let link = if cfg.link.trim().is_empty() {
        text.trim().to_string()
    } else {
        fill_pattern(&cfg.link, &json)
    };
    if link.is_empty() {
        return Err("the image host's answer had no link".into());
    }
    Ok((link, fill_pattern(&cfg.delete, &json)))
}

/// An image as PNG bytes.
pub fn png_of(img: &egui::ColorImage) -> Result<Vec<u8>, String> {
    let [w, h] = img.size;
    let px: Vec<u8> = img
        .pixels
        .iter()
        .flat_map(|c| c.to_srgba_unmultiplied())
        .collect();
    let buf = image::RgbaImage::from_raw(w as u32, h as u32, px).ok_or("bad image")?;
    let mut out = std::io::Cursor::new(Vec::new());
    buf.write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

pub const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

/// Start with Windows (HKCU Run key), pointing at the installed Yaperino.
pub fn set_start_with_windows(on: bool) -> Result<(), String> {
    let mut cmd = std::process::Command::new("reg");
    if on {
        let exe = crate::install::installed_exe()
            .filter(|p| p.is_file())
            .ok_or(
                "Install Yaperino first (Settings > About), so Windows starts the installed copy",
            )?;
        cmd.args([
            "add",
            RUN_KEY,
            "/v",
            crate::APP_ID,
            "/t",
            "REG_SZ",
            "/d",
            &format!("\"{}\"", exe.display()),
            "/f",
        ]);
    } else {
        cmd.args(["delete", RUN_KEY, "/v", crate::APP_ID, "/f"]);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let ok = cmd.status().map(|s| s.success()).unwrap_or(false);
    if ok || !on {
        Ok(())
    } else {
        Err("Windows refused the change".into())
    }
}

impl LiveApp {
    /// Settings from older versions: plain ignored phrases become ignore rules.
    pub(super) fn migrate_settings(&mut self) {
        if self.settings.ignore_rules.is_empty() && !self.settings.ignore_phrases.is_empty() {
            self.settings.ignore_rules = self
                .settings
                .ignore_phrases
                .drain(..)
                .filter(|p| !p.trim().is_empty())
                .map(|p| IgnoreRule {
                    pattern: p,
                    block: true,
                    ..Default::default()
                })
                .collect();
        }
    }

    /// Compiles highlights, ignores and filters again when Settings changed them.
    pub(super) fn refresh_matchers(&mut self) {
        let s = &self.settings;
        let key = serde_json::to_string(&(
            &s.highlight_phrases,
            &s.highlight_users,
            &s.highlight_badges,
            &s.highlight_blacklist,
            &s.ignore_rules,
            &s.ignore_users,
            &s.filters,
        ))
        .unwrap_or_default();
        if key == self.matchers_key {
            return;
        }
        let rule = |r: &HighlightRule, user: bool| -> Option<Rule> {
            if !r.enabled {
                return None;
            }
            let re = if user {
                rules::user_regex(&r.pattern, r.regex, r.case_sensitive)
            } else {
                rules::pattern_regex(&r.pattern, r.regex, r.case_sensitive)
            }?;
            Some(Rule {
                re,
                color: r.color,
                flash: r.flash,
                sound: r.sound,
            })
        };
        let m = Matchers {
            phrases: s
                .highlight_phrases
                .iter()
                .filter_map(|r| rule(r, false))
                .collect(),
            users: s
                .highlight_users
                .iter()
                .filter_map(|r| rule(r, true))
                .collect(),
            badges: s
                .highlight_badges
                .iter()
                .filter(|r| r.enabled && !r.pattern.trim().is_empty())
                .map(|r| {
                    (
                        r.pattern.trim().to_lowercase(),
                        Hit {
                            color: r.color,
                            flash: r.flash,
                            sound: r.sound,
                        },
                    )
                })
                .collect(),
            blacklist: s
                .highlight_blacklist
                .iter()
                .map(|u| u.trim().to_lowercase())
                .filter(|u| !u.is_empty())
                .collect(),
            ignores: s
                .ignore_rules
                .iter()
                .filter_map(|r| {
                    rules::pattern_regex(&r.pattern, r.regex, r.case_sensitive).map(|re| Ignore {
                        re,
                        block: r.block,
                        replacement: r.replacement.clone(),
                    })
                })
                .collect(),
            ignore_users: s
                .ignore_users
                .iter()
                .filter_map(|u| rules::user_regex(u, false, false))
                .collect(),
            filters: s
                .filters
                .iter()
                .map(|f| (f.name.clone(), rules::parse_filter(&f.filter)))
                .collect(),
        };
        self.matchers = Arc::new(m);
        self.matchers_key = key;
    }

    /// Ignores: None = drop the message; otherwise the message, its text maybe replaced.
    pub(super) fn apply_ignores(&self, line: ChatLine) -> Option<ChatLine> {
        let ChatLine::Msg {
            name,
            text,
            own: false,
            ..
        } = &line
        else {
            return Some(line);
        };
        if self.matchers.user_ignored(name) {
            return None;
        }
        let new = rules::apply_ignores(text, &self.matchers.ignores)?;
        if new == *text {
            return Some(line);
        }
        let mut line = line;
        if let ChatLine::Msg {
            text, emotes, segs, ..
        } = &mut line
        {
            *text = new;
            // Twitch's emote positions point into the old text
            emotes.clear();
            segs.clear();
        }
        Some(line)
    }

    /// Chatterino Highlights: (counts as a mention, the highlight).
    pub(super) fn highlight_hit(&self, line: &ChatLine) -> (bool, Option<Hit>) {
        let ChatLine::Msg {
            name,
            text,
            own: false,
            badges,
            ..
        } = line
        else {
            return (false, None);
        };
        // /hide: hidden people don't ping or flash either
        if self
            .settings
            .hidden_users
            .iter()
            .any(|u| u.eq_ignore_ascii_case(name))
        {
            return (false, None);
        }
        let t = text.to_lowercase();
        let me = self.account.as_ref().map(|a| a.login.to_lowercase());
        let mention = self.settings.highlight_mentions
            && !self.matchers.blacklist.contains(&name.to_lowercase())
            && me.as_ref().is_some_and(|m| {
                t.split(|c: char| !c.is_alphanumeric() && c != '_')
                    .any(|w| w == m)
            });
        match self.matchers.highlight(name, text, badges) {
            Some(h) => (true, Some(h)),
            None => (mention, None),
        }
    }

    /// Flash and sound for a highlighted message (sound while focused only when allowed).
    pub(super) fn signal_highlight(&self, hit: Hit) {
        let focused = self.ctx.input(|i| i.viewport().focused.unwrap_or(true));
        if hit.flash && !focused {
            self.ctx.send_viewport_cmd_to(
                egui::ViewportId::ROOT,
                egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Informational),
            );
        }
        if hit.sound && (!focused || self.settings.highlight_sound_focused) {
            play_sound();
        }
    }

    /// Moderation > Logs: appends a message to <logs>/<channel>/<channel>-<date>.log.
    pub(super) fn log_line(&self, channel: &str, line: &ChatLine) {
        if !self.settings.logging || self.demo {
            return;
        }
        if self.settings.log_only_listed
            && !self
                .settings
                .log_channels
                .iter()
                .any(|c| c.trim().eq_ignore_ascii_case(channel))
        {
            return;
        }
        let dir = if self.settings.log_dir.trim().is_empty() {
            default_log_dir()
        } else {
            PathBuf::from(self.settings.log_dir.trim())
        };
        let now = chrono::Local::now();
        let stamp = now.format(if self.settings.log_seconds {
            "%H:%M:%S"
        } else {
            "%H:%M"
        });
        let text = match line {
            ChatLine::Msg {
                name,
                text,
                action,
                platform,
                ..
            } => {
                let tag = if *platform == Platform::Twitch {
                    String::new()
                } else {
                    format!("[{}] ", platform.tag())
                };
                if *action {
                    format!("[{stamp}] {tag}{name} {text}")
                } else {
                    format!("[{stamp}] {tag}{name}: {text}")
                }
            }
            ChatLine::System(t) => format!("[{stamp}] {t}"),
            ChatLine::AutoMod(h) => format!("[{stamp}] {}", h.summary()),
        };
        let name = safe_name(channel);
        let folder = dir.join(&name);
        if std::fs::create_dir_all(&folder).is_ok() {
            use std::io::Write;
            let file = folder.join(format!("{name}-{}.log", now.format("%Y-%m-%d")));
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(file)
            {
                let _ = writeln!(f, "{text}");
            }
        }
    }

    /// Always on top (General > Interface), applied when it changes.
    pub(super) fn apply_window_options(&mut self, ctx: &egui::Context) {
        if self.shot.is_some() {
            return;
        }
        if self.applied_top != Some(self.settings.always_on_top) {
            let level = if self.settings.always_on_top {
                egui::WindowLevel::AlwaysOnTop
            } else {
                egui::WindowLevel::Normal
            };
            ctx.send_viewport_cmd_to(
                egui::ViewportId::ROOT,
                egui::ViewportCommand::WindowLevel(level),
            );
            self.applied_top = Some(self.settings.always_on_top);
        }
    }

    /// Reads the plugin folder and starts the switched-on plugins.
    pub(super) fn load_plugins(&mut self) {
        self.plugin_list = crate::plugins::scan();
        if !self.settings.plugins_enabled {
            for p in &self.plugin_list {
                self.plugins.unload(&p.id);
            }
            return;
        }
        for p in self.plugin_list.clone() {
            if self.settings.enabled_plugins.contains(&p.id)
                && p.problem.is_none()
                && !self.plugins.is_loaded(&p.id)
            {
                let _ = self.plugins.load(&p);
            }
        }
    }

    /// Moderation (Twitch): times out, bans, unbans or deletes in a split's channel, then
    /// says how it went in that chat.
    pub(super) fn moderate(&mut self, group: &str, m: crate::moderation::ModAction) {
        let channel = self
            .chats
            .iter()
            .find(|t| t.login == group)
            .and_then(|t| t.twitch.clone())
            .unwrap_or_else(|| tab_primary(group).1);
        let Some(room) = self.rooms.get(&channel).cloned() else {
            self.push_to(
                group,
                ChatLine::System(
                    "Moderation: this channel's Twitch id isn't known yet, try again in a moment"
                        .into(),
                ),
            );
            return;
        };
        let Some(a) = self.account.as_ref().filter(|a| a.can_moderate()) else {
            self.push_to(group, ChatLine::System("Moderation: sign in to Twitch again (Settings > Accounts) to give Yaperino moderation rights".into()));
            return;
        };
        let auth = crate::moderation::ModAuth {
            client_id: a.client_id.clone(),
            token: a.token.clone(),
            user_id: a.user_id.clone(),
        };
        let (tx, ctx, g) = (self.side_tx.clone(), self.ctx.clone(), group.to_string());
        std::thread::spawn(move || {
            let result = crate::moderation::run(&auth, &room, &m);
            let text = match &result {
                Ok(()) => format!("You {}.", m.describe()),
                Err(e) => format!("Moderation failed: {e}"),
            };
            let _ = tx.send(SideMsg::Line(g, ChatLine::System(text)));
            // a pin changed: show the new one right away
            if result.is_ok()
                && matches!(
                    m.kind,
                    crate::moderation::ModKind::Pin { .. } | crate::moderation::ModKind::Unpin(_)
                )
            {
                let pin = crate::moderation::get_pin(&auth, &room).ok().flatten();
                let _ = tx.send(SideMsg::Pin(channel, pin));
            }
            ctx.request_repaint();
        });
    }

    /// A Twitch mod / broadcaster command (/timeout, /slow, /poll, /raid ...) in a channel,
    /// through Twitch's API; the answer becomes a line in the chat.
    pub(super) fn mod_command(&mut self, group: &str, cmd: crate::moderation::ModCommand) {
        let channel = self
            .chats
            .iter()
            .find(|t| t.login == group)
            .and_then(|t| t.twitch.clone())
            .unwrap_or_else(|| tab_primary(group).1);
        let Some(room) = self.rooms.get(&channel).cloned() else {
            self.push_to(
                group,
                ChatLine::System(
                    "Mod command: this channel's Twitch id isn't known yet, try again in a moment"
                        .into(),
                ),
            );
            return;
        };
        let Some(a) = self.account.as_ref() else {
            self.push_to(
                group,
                ChatLine::System(
                    "Mod command: sign in to Twitch first (Settings > Accounts)".into(),
                ),
            );
            return;
        };
        let scope = crate::moderation::required_scope(&cmd);
        if !a.scopes.is_empty() && !a.has_scope(scope) {
            self.push_to(
                group,
                ChatLine::System(format!("Mod command: your Twitch sign-in doesn't allow this yet ({scope}). Sign in again in Settings > Accounts and allow the new permissions.")),
            );
            return;
        }
        let auth = crate::moderation::ModAuth {
            client_id: a.client_id.clone(),
            token: a.token.clone(),
            user_id: a.user_id.clone(),
        };
        let (tx, ctx, g) = (self.side_tx.clone(), self.ctx.clone(), group.to_string());
        std::thread::spawn(move || {
            let text = match crate::moderation::run_command(&auth, &room, &cmd) {
                Ok(done) => done,
                Err(e) => format!("Mod command failed: {e}"),
            };
            let _ = tx.send(SideMsg::Line(g, ChatLine::System(text)));
            ctx.request_repaint();
        });
    }

    /// Each frame: plugin timers and finished HTTP requests; their messages go to the chat
    /// they name.
    pub(super) fn poll_plugins(&mut self) {
        if !self.settings.plugins_enabled {
            return;
        }
        for o in self.plugins.poll() {
            let (crate::plugins::Out::System(channel, text)
            | crate::plugins::Out::Send(channel, text)) = &o;
            let group = self
                .chats
                .iter()
                .find(|t| {
                    tab_primary(&t.login).1.eq_ignore_ascii_case(channel)
                        || t.twitch
                            .as_deref()
                            .is_some_and(|tw| tw.eq_ignore_ascii_case(channel))
                })
                .map(|t| t.login.clone());
            let Some(group) = group else { continue };
            match o {
                crate::plugins::Out::System(_, _) => {
                    self.push_to(&group, ChatLine::System(text.clone()))
                }
                crate::plugins::Out::Send(_, _) => {
                    let p = tab_primary(&group).0;
                    self.send_message(group.clone(), text.clone(), p);
                }
            }
        }
    }

    /// A plugin command typed into a split: runs it and does what the plugin asked.
    /// Returns false when the text isn't a plugin command.
    pub(super) fn run_plugin_command(
        &mut self,
        group: &str,
        text: &str,
        platform: Platform,
    ) -> bool {
        if !self.settings.plugins_enabled {
            return false;
        }
        let channel = tab_primary(group).1;
        let Some(out) = self.plugins.run_command(text, &channel) else {
            return false;
        };
        for o in out {
            match o {
                crate::plugins::Out::System(_, t) => self.push_to(group, ChatLine::System(t)),
                crate::plugins::Out::Send(_, t) => {
                    self.send_message(group.to_string(), t, platform)
                }
            }
        }
        true
    }
}
