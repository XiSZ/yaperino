//! Opening a stream in Streamlink (like Chatty's Livestreamer/Streamlink dialog).
//! Each argument is passed on its own (never through a shell), and no token is
//! ever put on the command line.

use crate::platforms::{youtube_url, Platform};
use std::path::PathBuf;
use std::process::Command;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct StreamlinkSettings {
    /// Empty = find it automatically.
    pub path: String,
    pub quality: String,
    /// Extra arguments, split on spaces (e.g. `--player "C:\...\mpv.exe"` is not supported; use `--player=...`).
    pub extra_args: String,
}

impl Default for StreamlinkSettings {
    fn default() -> Self {
        Self {
            path: String::new(),
            quality: "best".into(),
            extra_args: String::new(),
        }
    }
}

/// The settings path, else `streamlinkw.exe` (no console window) or `streamlink.exe`
/// in Program Files, else on PATH.
pub fn find(settings: &StreamlinkSettings) -> Option<PathBuf> {
    if !settings.path.trim().is_empty() {
        let p = PathBuf::from(settings.path.trim());
        return p.is_file().then_some(p);
    }
    let mut dirs: Vec<PathBuf> = Vec::new();
    for var in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
        if let Some(base) = std::env::var_os(var) {
            let base = PathBuf::from(base);
            dirs.push(base.join("Streamlink").join("bin"));
            dirs.push(base.join("Programs").join("Streamlink").join("bin"));
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    for d in &dirs {
        let names: &[&str] = if cfg!(windows) {
            &["streamlinkw.exe", "streamlink.exe"]
        } else {
            &["streamlink"]
        };
        for name in names {
            let p = d.join(name);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// The page Streamlink should open for a channel on a platform.
pub fn url(platform: Platform, channel: &str, youtube_video: Option<&str>) -> String {
    let c = channel.trim();
    match platform {
        Platform::Twitch => format!("https://www.twitch.tv/{}", c.to_lowercase()),
        Platform::Kick => format!("https://kick.com/{}", c.to_lowercase()),
        Platform::YouTube => match youtube_video {
            Some(v) => format!("https://www.youtube.com/watch?v={v}"),
            None => format!("{}/live", youtube_url(c)),
        },
        Platform::TikTok => format!("https://www.tiktok.com/@{}/live", c.trim_start_matches('@')),
    }
}

pub fn args(settings: &StreamlinkSettings, url: &str) -> Vec<String> {
    let mut a: Vec<String> = settings
        .extra_args
        .split_whitespace()
        .map(String::from)
        .collect();
    a.push(url.to_string());
    a.push(if settings.quality.trim().is_empty() {
        "best".into()
    } else {
        settings.quality.trim().to_string()
    });
    a
}

pub fn open(settings: &StreamlinkSettings, url: &str) -> Result<(), String> {
    let exe = find(settings).ok_or("Streamlink was not found. Install it from streamlink.github.io or set its path in Settings > Streamlink.")?;
    let mut cmd = Command::new(&exe);
    cmd.args(args(settings, url));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // streamlink.exe is a console program; keep its window away
        if exe
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("streamlink.exe"))
        {
            cmd.creation_flags(0x0800_0000);
        }
    }
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("Couldn't start Streamlink: {e}"))
}

/// The plugin name Streamlink uses for a platform.
pub fn plugin_name(p: Platform) -> &'static str {
    match p {
        Platform::Twitch => "twitch",
        Platform::Kick => "kick",
        Platform::YouTube => "youtube",
        Platform::TikTok => "tiktok",
    }
}

static PLUGINS: std::sync::OnceLock<Option<Vec<String>>> = std::sync::OnceLock::new();
static ASKED: std::sync::Once = std::sync::Once::new();

/// Parses `streamlink --plugins` ("Loaded plugins: a, b, c").
pub fn parse_plugins(text: &str) -> Vec<String> {
    let list = text
        .lines()
        .filter(|l| !l.starts_with('['))
        .collect::<Vec<_>>()
        .join(" ");
    let list = list.split_once(':').map(|(_, r)| r).unwrap_or(&list);
    list.split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Whether the installed Streamlink has a plugin for the platform: Some(yes/no), or
/// None while it is still being asked (once, in the background) or not installed.
pub fn supports(settings: &StreamlinkSettings, p: Platform) -> Option<bool> {
    ASKED.call_once(|| {
        let s = settings.clone();
        std::thread::spawn(move || {
            let r = find(&s).and_then(|exe| {
                let mut cmd = Command::new(exe);
                cmd.arg("--plugins");
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    cmd.creation_flags(0x0800_0000);
                }
                cmd.output()
                    .ok()
                    .map(|o| parse_plugins(&String::from_utf8_lossy(&o.stdout)))
            });
            let _ = PLUGINS.set(r.filter(|l| !l.is_empty()));
        });
    });
    PLUGINS
        .get()
        .and_then(|o| o.as_ref())
        .map(|l| l.iter().any(|n| n == plugin_name(p)))
}

/// What `supports` found so far, without asking.
pub fn known_support(p: Platform) -> Option<bool> {
    PLUGINS
        .get()
        .and_then(|o| o.as_ref())
        .map(|l| l.iter().any(|n| n == plugin_name(p)))
}

/// `streamlink --version` (for the Settings page and tests; opens no player).
pub fn version(settings: &StreamlinkSettings) -> Result<String, String> {
    let exe = find(settings).ok_or("not found")?;
    let mut cmd = Command::new(&exe);
    cmd.arg("--version");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().map_err(|e| e.to_string())?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(if s.is_empty() {
        String::from_utf8_lossy(&out.stderr).trim().to_string()
    } else {
        s
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_and_args() {
        assert_eq!(
            url(Platform::Twitch, "XQC", None),
            "https://www.twitch.tv/xqc"
        );
        assert_eq!(url(Platform::Kick, "xqc", None), "https://kick.com/xqc");
        assert_eq!(
            url(Platform::YouTube, "@LofiGirl", None),
            "https://www.youtube.com/@LofiGirl/live"
        );
        assert_eq!(
            url(Platform::YouTube, "@x", Some("abc")),
            "https://www.youtube.com/watch?v=abc"
        );
        assert_eq!(
            url(Platform::TikTok, "@user", None),
            "https://www.tiktok.com/@user/live"
        );
        let s = StreamlinkSettings {
            extra_args: "--twitch-low-latency".into(),
            ..Default::default()
        };
        assert_eq!(args(&s, "u"), vec!["--twitch-low-latency", "u", "best"]);
    }

    #[test]
    fn reads_plugin_list() {
        let out = "[session][info] Plugin twitch is being overridden by x.py\nLoaded plugins: abematv, kick, tiktok, twitch, youtube\n";
        let l = parse_plugins(out);
        assert!(
            l.contains(&"kick".to_string())
                && l.contains(&"tiktok".to_string())
                && l.contains(&"youtube".to_string())
        );
        assert!(!l.iter().any(|n| n.contains("overridden")));
    }

    /// Runs `streamlink --version` only (no player). `cargo test -- --ignored streamlink_version --nocapture`
    #[test]
    #[ignore]
    fn streamlink_version() {
        println!("{:?}", find(&StreamlinkSettings::default()));
        println!("{}", version(&StreamlinkSettings::default()).unwrap());
    }
}
