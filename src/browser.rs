//! Opening web pages (channel pages, players, sign-in pages) in the default browser.

/// Windows: asks the shell to open the address, as a double-click on a link would.
/// (Not through `cmd /C start`: cmd treats `&` as "run another command" and cut every
/// address at its first `&`, so e.g. the Twitch player lost `&parent=twitch.tv` and
/// showed "This embed is misconfigured".)
#[cfg(windows)]
pub fn open_url(url: &str) {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let wide = |s: &str| -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() };
    let (verb, file) = (wide("open"), wide(url));
    // SAFETY: both strings are NUL-terminated UTF-16 that live until the call returns;
    // the window and the other strings may be null.
    unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        );
    }
}

#[cfg(target_os = "macos")]
pub fn open_url(url: &str) {
    let _ = std::process::Command::new("open").arg(url).spawn();
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn open_url(url: &str) {
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}
