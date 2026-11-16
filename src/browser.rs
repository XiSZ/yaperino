//! Opening web pages (channel pages, players, sign-in pages) in the default browser.

use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn open_url(url: &str) {
    let mut cmd = Command::new("cmd");
    cmd.args(["/C", "start", "", url]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let _ = cmd.spawn();
}
