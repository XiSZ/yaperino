//! Where Yaperino keeps its files on each system.
//!
//! - Windows: `%APPDATA%\Yaperino` (settings in its `data` folder, plugins, logs, exports)
//!   and `%LOCALAPPDATA%\Yaperino` (sign-in browser profiles, the notification icon).
//! - Linux and macOS: one folder, the one eframe saves the settings in
//!   (`~/.local/share/yaperino`, `~/Library/Application Support/Yaperino`), so everything
//!   stays together.

use std::path::PathBuf;

/// Settings, plugins, logs and exports.
pub fn app_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join(crate::APP_ID))
    }
    #[cfg(not(windows))]
    {
        eframe::storage_dir(crate::APP_ID)
    }
}

/// Files that belong to this computer only (not roamed on Windows).
pub fn local_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join(crate::APP_ID))
    }
    #[cfg(not(windows))]
    {
        app_dir()
    }
}

/// `app_dir()`, or a folder next to the program if the system has none.
pub fn app_dir_or_here() -> PathBuf {
    app_dir().unwrap_or_else(|| PathBuf::from(crate::APP_ID))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn windows_paths_are_unchanged() {
        let appdata = PathBuf::from(std::env::var_os("APPDATA").unwrap());
        let local = PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap());
        assert_eq!(app_dir().unwrap(), appdata.join("Yaperino"));
        assert_eq!(local_dir().unwrap(), local.join("Yaperino"));
        // eframe keeps the settings in the "data" folder inside app_dir()
        assert_eq!(
            eframe::storage_dir(crate::APP_ID).unwrap(),
            appdata.join("Yaperino").join("data")
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn one_folder_with_the_settings() {
        assert_eq!(app_dir(), eframe::storage_dir(crate::APP_ID));
        assert_eq!(local_dir(), app_dir());
    }
}
