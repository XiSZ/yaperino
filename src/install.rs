//! Installing Yaperino for this Windows user (Settings > About), so it shows up in
//! Start and in Settings > Apps > Installed apps like any app, without admin rights:
//! - the program is copied to %LOCALAPPDATA%\Programs\Yaperino\Yaperino.exe,
//! - a Start menu shortcut %APPDATA%\Microsoft\Windows\Start Menu\Programs\Yaperino.lnk,
//! - an Apps list entry under HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Yaperino
//!   whose Uninstall runs `Yaperino.exe --uninstall`.
//! Settings and sign-ins (%APPDATA%\Yaperino) are shared with the copy and kept on uninstall.

use std::path::PathBuf;
use std::process::Command;

pub const UNINSTALL_KEY: &str =
    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Yaperino";

fn no_window(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}

pub fn install_dir() -> Option<PathBuf> {
    Some(
        PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
            .join("Programs")
            .join(crate::APP_ID),
    )
}

pub fn installed_exe() -> Option<PathBuf> {
    install_dir().map(|d| d.join(format!("{}.exe", crate::APP_ID)))
}

pub fn shortcut() -> Option<PathBuf> {
    Some(
        PathBuf::from(std::env::var_os("APPDATA")?)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
            .join(format!("{}.lnk", crate::APP_ID)),
    )
}

/// Installed for this user (the copy and its Apps list entry are there).
pub fn is_installed() -> bool {
    installed_exe().is_some_and(|p| p.is_file())
        && no_window(Command::new("reg").args(["query", UNINSTALL_KEY]))
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
}

/// Is this program the installed copy?
pub fn running_installed() -> bool {
    match (std::env::current_exe().ok(), installed_exe()) {
        (Some(a), Some(b)) => a.eq_ignore_ascii_case_path(&b),
        _ => false,
    }
}

trait PathEq {
    fn eq_ignore_ascii_case_path(&self, other: &std::path::Path) -> bool;
}
impl PathEq for PathBuf {
    fn eq_ignore_ascii_case_path(&self, other: &std::path::Path) -> bool {
        self.to_string_lossy()
            .eq_ignore_ascii_case(&other.to_string_lossy())
    }
}

/// A PowerShell single-quoted string.
fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// The reg.exe lines that make the Apps list entry.
pub fn registry_values(dir: &str, exe: &str, size_kb: u64) -> Vec<(String, &'static str, String)> {
    vec![
        ("DisplayName".into(), "REG_SZ", crate::APP_ID.into()),
        ("DisplayIcon".into(), "REG_SZ", format!("{exe},0")),
        ("DisplayVersion".into(), "REG_SZ", crate::VERSION.into()),
        ("Publisher".into(), "REG_SZ", crate::APP_ID.into()),
        ("InstallLocation".into(), "REG_SZ", dir.into()),
        (
            "UninstallString".into(),
            "REG_SZ",
            format!("\"{exe}\" --uninstall"),
        ),
        ("NoModify".into(), "REG_DWORD", "1".into()),
        ("NoRepair".into(), "REG_DWORD", "1".into()),
        ("EstimatedSize".into(), "REG_DWORD", size_kb.to_string()),
    ]
}

/// Copies the program, makes the Start menu shortcut and the Apps list entry.
/// Running it again updates the copy (the installed copy must be closed).
pub fn install() -> Result<String, String> {
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = install_dir().ok_or("no LOCALAPPDATA folder")?;
    let exe = installed_exe().ok_or("no LOCALAPPDATA folder")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    if !running_installed() {
        std::fs::copy(&me, &exe).map_err(|e| {
            format!("Couldn't copy the program (is the installed Yaperino open?): {e}")
        })?;
    }
    let ico = dir.join("yaperino.ico");
    let _ = std::fs::write(&ico, crate::logo::ico(&crate::logo::ICO_SIZES));

    // Start menu shortcut (Windows' own WScript.Shell)
    let lnk = shortcut().ok_or("no APPDATA folder")?;
    let script = format!(
        "$s=(New-Object -ComObject WScript.Shell).CreateShortcut({}); $s.TargetPath={}; $s.WorkingDirectory={}; $s.IconLocation={}; $s.Description='Chat for Twitch, Kick, YouTube and TikTok'; $s.Save()",
        ps_quote(&lnk.to_string_lossy()),
        ps_quote(&exe.to_string_lossy()),
        ps_quote(&dir.to_string_lossy()),
        ps_quote(&format!("{},0", exe.to_string_lossy())),
    );
    let ok = no_window(Command::new("powershell").args([
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        &script,
    ]))
    .status()
    .map(|s| s.success())
    .unwrap_or(false);
    if !ok {
        return Err("Couldn't make the Start menu shortcut".into());
    }

    // the Apps list entry (per user, no admin)
    let size_kb = std::fs::metadata(&exe).map(|m| m.len() / 1024).unwrap_or(0);
    for (name, kind, data) in
        registry_values(&dir.to_string_lossy(), &exe.to_string_lossy(), size_kb)
    {
        let ok = no_window(Command::new("reg").args([
            "add",
            UNINSTALL_KEY,
            "/v",
            &name,
            "/t",
            kind,
            "/d",
            &data,
            "/f",
        ]))
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
        if !ok {
            return Err(format!("Couldn't write the Apps list entry ({name})"));
        }
    }
    Ok(format!(
        "Installed: Yaperino is in Start and in Settings > Apps ({})",
        exe.display()
    ))
}

/// Removes the shortcut, the Apps list entry and the installed copy (after this
/// program has closed). Settings and sign-ins stay.
pub fn uninstall() -> Result<(), String> {
    if let Some(l) = shortcut() {
        let _ = std::fs::remove_file(l);
    }
    let _ = no_window(Command::new("reg").args(["delete", UNINSTALL_KEY, "/f"])).status();
    if let Some(dir) = install_dir().filter(|d| d.is_dir()) {
        if running_installed() {
            // the running copy can't delete itself: a hidden helper waits, then removes the folder
            let _ = no_window(Command::new("cmd").args([
                "/C",
                &format!(
                    "ping -n 3 127.0.0.1 >nul & rmdir /s /q \"{}\"",
                    dir.display()
                ),
            ]))
            .spawn();
        } else {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apps_list_entry() {
        let v = registry_values(
            r"C:\Users\x\AppData\Local\Programs\Yaperino",
            r"C:\Users\x\AppData\Local\Programs\Yaperino\Yaperino.exe",
            12345,
        );
        let get = |n: &str| {
            v.iter()
                .find(|(k, _, _)| k == n)
                .map(|(_, _, d)| d.clone())
                .unwrap()
        };
        assert_eq!(get("DisplayName"), "Yaperino");
        assert_eq!(
            get("UninstallString"),
            r#""C:\Users\x\AppData\Local\Programs\Yaperino\Yaperino.exe" --uninstall"#
        );
        assert!(get("DisplayIcon").ends_with("Yaperino.exe,0"));
        assert_eq!(ps_quote("it's"), "'it''s'");
    }

    #[test]
    fn logo_and_icon() {
        let px = crate::logo::rgba(32);
        assert_eq!(px.len(), 32 * 32 * 4);
        assert_eq!(px[3], 0, "corner is transparent");
        let mid = ((16 * 32 + 16) * 4) as usize;
        assert_eq!(px[mid + 3], 255, "centre is solid");
        let ico = crate::logo::ico(&[16, 32]);
        assert_eq!(&ico[0..6], &[0, 0, 1, 0, 2, 0]);
    }
}
