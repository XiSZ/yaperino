//! Opening web pages (channel pages, players, sign-in pages) in the default browser.

use std::process::Command;

#[cfg(windows)]
pub fn open_url(url: &str) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut cmd = Command::new("cmd");
    cmd.args(["/C", "start", "", url]);
    cmd.creation_flags(CREATE_NO_WINDOW);
    let _ = cmd.spawn();
}

#[cfg(target_os = "macos")]
pub fn open_url(url: &str) {
    let _ = Command::new("open").arg(url).spawn();
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn open_url(url: &str) {
    let _ = Command::new("xdg-open").arg(url).spawn();
}
