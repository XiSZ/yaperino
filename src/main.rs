#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Yaperino: a Chatterino-style chat for Twitch, Kick, YouTube and TikTok, with a
//! Chatty-style list of the followed channels that are live (Alt+L).
//!
//! Start options:
//! - `--screenshot <file.png>`: draw sample data once and save a picture (for checks)
//! - `--web-login kick|youtube`: the sign-in window (see weblogin.rs)
//! - `--write-logo <folder>`: write yaperino.png and yaperino.ico there
//! - `--uninstall`: remove the Start menu entry and the Apps list entry (see install.rs)

mod account;
mod app;
mod browser;
mod chat;
mod demo;
mod emotes;
mod eventsub;
mod fetch;
mod hotkeys;
mod install;
mod kick_auth;
mod logo;
mod model;
mod moderation;
mod modlog;
mod namehistory;
mod otherchat;
mod paths;
mod platforms;
mod plugins;
mod rules;
mod spell;
mod streamlink;
mod style;
mod tiktokchat;
mod tray;
mod weblogin;
mod youtube_auth;

use std::path::PathBuf;

/// The app's name (also its settings folder: %APPDATA%\Yaperino).
pub const APP_ID: &str = "Yaperino";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Settings saved by an earlier test build ("live-streams") move to the new folder once.
fn move_old_settings() {
    let Some(appdata) = std::env::var_os("APPDATA").map(PathBuf::from) else {
        return;
    };
    let new_dir = appdata.join(APP_ID).join("data");
    let new = new_dir.join("app.ron");
    if new.exists() {
        return;
    }
    for old_name in ["live-streams"] {
        let old = appdata.join(old_name).join("data").join("app.ron");
        if old.is_file() && std::fs::create_dir_all(&new_dir).is_ok() {
            let _ = std::fs::copy(&old, &new);
            return;
        }
    }
}

/// The window icon (the logo at 64 px).
pub fn icon() -> egui::IconData {
    egui::IconData {
        rgba: logo::rgba(64),
        width: 64,
        height: 64,
    }
}

fn write_logo(dir: &str) -> std::io::Result<()> {
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("yaperino.ico"), logo::ico(&logo::ICO_SIZES))?;
    let img = image::RgbaImage::from_raw(512, 512, logo::rgba(512)).expect("logo size");
    img.save(dir.join("yaperino.png"))
        .map_err(std::io::Error::other)
}

/// Started straight from the build folder (target\debug or target\release)? Then run a copy
/// from %LOCALAPPDATA%\Yaperino\run instead and exit, so the build's own program file is
/// never locked and the next build can always replace it. A copy per build (named by its
/// size and time) so an older copy that is still running doesn't get in the way.
fn run_from_copy(args: &[String]) -> bool {
    if std::env::var_os("YAPERINO_IN_PLACE").is_some() {
        return false;
    }
    let Ok(me) = std::env::current_exe() else {
        return false;
    };
    let in_build = me
        .components()
        .any(|c| c.as_os_str().eq_ignore_ascii_case("target"));
    if !in_build {
        return false;
    }
    let Some(dir) =
        std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join(APP_ID).join("run"))
    else {
        return false;
    };
    let Ok(meta) = std::fs::metadata(&me) else {
        return false;
    };
    let stamp = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let copy = dir.join(format!("{APP_ID}-{}-{stamp}.exe", meta.len()));
    if std::fs::create_dir_all(&dir).is_err() {
        return false;
    }
    if !copy.is_file() && std::fs::copy(&me, &copy).is_err() {
        return false;
    }
    // older copies that are no longer running
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            if e.path() != copy {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    std::process::Command::new(&copy)
        .args(args.iter().skip(1))
        .spawn()
        .is_ok()
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    // checks (--screenshot) and helper starts run where they are; the app itself runs from a copy
    let special = ["--screenshot", "--web-login", "--write-logo", "--uninstall"];
    if !args.iter().any(|a| special.contains(&a.as_str())) && run_from_copy(&args) {
        return Ok(());
    }
    // the sign-in window for Kick / YouTube (see weblogin.rs)
    if let Some(i) = args.iter().position(|a| a == "--web-login") {
        std::process::exit(weblogin::run(
            args.get(i + 1).map(String::as_str).unwrap_or(""),
        ));
    }
    if let Some(i) = args.iter().position(|a| a == "--write-logo") {
        let r = write_logo(args.get(i + 1).map(String::as_str).unwrap_or("."));
        std::process::exit(if r.is_ok() { 0 } else { 1 });
    }
    if args.iter().any(|a| a == "--uninstall") {
        std::process::exit(if install::uninstall().is_ok() { 0 } else { 1 });
    }
    let shot = args
        .iter()
        .position(|a| a == "--screenshot")
        .and_then(|i| args.get(i + 1))
        .map(|p| app::Shot::new(PathBuf::from(p)));

    let mut viewport = egui::ViewportBuilder::default()
        .with_title(APP_ID)
        .with_app_id(APP_ID)
        // Chatterino-style own title bar (Settings > Customization can bring Windows' back)
        .with_decorations(false)
        .with_icon(icon())
        .with_inner_size([1570.0, 950.0])
        .with_min_inner_size([360.0, 240.0]);
    if shot.is_some() {
        // never steal focus from what the person is doing during a check
        viewport = viewport.with_active(false).with_position([20.0, 20.0]);
        #[cfg(windows)]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }
    let options = eframe::NativeOptions {
        viewport,
        persist_window: shot.is_none(),
        run_and_return: true,
        ..Default::default()
    };
    if shot.is_none() {
        move_old_settings();
    }
    eframe::run_native(
        APP_ID,
        options,
        Box::new(move |cc| Ok(Box::new(app::LiveApp::new(cc, shot)))),
    )
}
