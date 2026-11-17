//! Moltorino's Settings Manager: back up the settings to a file, restore one, and a
//! automatic daily backup (the last 10 are kept). Backups never hold sign-ins: Twitch
//! accounts, the Twitch Client ID, Kick and YouTube sign-ins are left out, and a restore
//! keeps the ones in use. A child of `app`.

use super::*;

/// How many automatic backups are kept.
const KEEP_AUTO: usize = 10;

/// Where backups go: <Yaperino's folder>/Backups.
pub fn backup_dir() -> PathBuf {
    crate::paths::app_dir_or_here().join("Backups")
}

/// The settings without anything that signs in.
fn without_sign_ins(s: &Settings) -> Settings {
    let mut s = s.clone();
    s.device = Default::default();
    s.twitch_accounts.clear();
    s.current_twitch.clear();
    s.twitch_client_id.clear();
    s.youtube = Default::default();
    s.kick = Default::default();
    s
}

/// The settings as a backup file's text (JSON), without sign-ins.
pub fn to_backup(s: &Settings) -> Result<String, String> {
    serde_json::to_string_pretty(&without_sign_ins(s)).map_err(|e| e.to_string())
}

/// Settings from a backup file's text, keeping the sign-ins of `current`.
pub fn from_backup(current: &Settings, text: &str) -> Result<Settings, String> {
    let mut s: Settings =
        serde_json::from_str(text).map_err(|e| format!("not a Yaperino settings backup ({e})"))?;
    s.device = current.device.clone();
    s.twitch_accounts = current.twitch_accounts.clone();
    s.current_twitch = current.current_twitch.clone();
    s.twitch_client_id = current.twitch_client_id.clone();
    s.youtube = current.youtube.clone();
    s.kick = current.kick.clone();
    Ok(s)
}

/// Writes a backup file ("auto" ones are the daily automatic backups); returns its path.
pub fn write_backup(s: &Settings, auto: bool) -> Result<PathBuf, String> {
    let dir = backup_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let stamp = chrono::Local::now().format("%Y-%m-%d_%H%M%S");
    let kind = if auto { "auto" } else { "manual" };
    let path = dir.join(format!("yaperino-settings-{stamp}-{kind}.json"));
    std::fs::write(&path, to_backup(s)?).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

/// The backup files there are, newest first.
pub fn list_backups() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(backup_dir())
        .map(|d| {
            d.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                        n.starts_with("yaperino-settings-") && n.ends_with(".json")
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    // the names start with the date and time, so they sort by age
    files.sort();
    files.reverse();
    files
}

/// The daily automatic backup: one when the newest automatic one is a day old (or there is
/// none); only the last KEEP_AUTO automatic ones are kept.
pub fn auto_backup(s: &Settings) {
    let autos: Vec<PathBuf> = list_backups()
        .into_iter()
        .filter(|p| p.to_string_lossy().ends_with("-auto.json"))
        .collect();
    let fresh = autos
        .first()
        .and_then(|p| p.metadata().ok())
        .and_then(|m| m.modified().ok())
        .is_some_and(|t| {
            t.elapsed()
                .is_ok_and(|age| age < std::time::Duration::from_secs(24 * 3600))
        });
    if fresh {
        return;
    }
    if write_backup(s, true).is_ok() {
        for old in list_backups()
            .into_iter()
            .filter(|p| p.to_string_lossy().ends_with("-auto.json"))
            .skip(KEEP_AUTO)
        {
            let _ = std::fs::remove_file(old);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backups_leave_sign_ins_out_and_restores_keep_them() {
        let mut mine = Settings::default();
        mine.twitch_client_id = "my-client-id".into();
        mine.current_twitch = "me".into();
        mine.kick.token = "kick-secret".into();
        mine.youtube.refresh_token = "yt-secret".into();
        mine.bold_names = !mine.bold_names;
        let text = to_backup(&mine).unwrap();
        for secret in ["my-client-id", "kick-secret", "yt-secret"] {
            assert!(!text.contains(secret), "{secret} is in the backup");
        }
        // restoring into other settings: their sign-ins stay, the rest comes from the backup
        let mut other = Settings::default();
        other.kick.token = "other-kick".into();
        other.twitch_client_id = "other-id".into();
        let restored = from_backup(&other, &text).unwrap();
        assert_eq!(restored.bold_names, mine.bold_names);
        assert_eq!(restored.kick.token, "other-kick");
        assert_eq!(restored.twitch_client_id, "other-id");
        assert!(from_backup(&other, "not json").is_err());
    }
}
