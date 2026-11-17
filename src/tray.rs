//! Hide to tray (Moltorino): with Settings > General "Close to the tray" on, closing the
//! window hides it, and a Yaperino icon by the clock brings it back (click it, or its menu's
//! "Show Yaperino"); the menu's "Quit" quits. Windows only so far.
//!
//! The window is shown and hidden through Windows itself: while it's hidden, eframe stops
//! running the app, so the tray's events can't wait for the app to act on them.

use std::sync::atomic::{AtomicBool, Ordering};

/// The tray menu's "Quit" was picked: the next close really quits.
pub static QUIT: AtomicBool = AtomicBool::new(false);

/// The tray icon; it goes away when this is dropped.
pub struct Tray {
    #[cfg(windows)]
    _icon: tray_icon::TrayIcon,
}

/// The main window's Windows handle (HWND), from eframe's frame.
pub fn window_id(frame: &eframe::Frame) -> Option<isize> {
    #[cfg(windows)]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        match frame.window_handle().ok()?.as_raw() {
            RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
            _ => None,
        }
    }
    #[cfg(not(windows))]
    {
        let _ = frame;
        None
    }
}

/// Hides the window (it stays running, in the tray).
pub fn hide(hwnd: isize) {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
        ShowWindow(hwnd as _, SW_HIDE);
    }
    #[cfg(not(windows))]
    let _ = hwnd;
}

/// Shows the window again and brings it to the front.
pub fn show(hwnd: isize) {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SetForegroundWindow, ShowWindow, SW_SHOW,
        };
        ShowWindow(hwnd as _, SW_SHOW);
        SetForegroundWindow(hwnd as _);
    }
    #[cfg(not(windows))]
    let _ = hwnd;
}

/// Creates the tray icon; its events show the window or quit. None when it can't be made
/// (then closing just quits, so the window never gets lost).
pub fn create(hwnd: isize, ctx: egui::Context) -> Option<Tray> {
    #[cfg(windows)]
    {
        use tray_icon::menu::{Menu, MenuEvent, MenuItem};
        use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
        let menu = Menu::new();
        let show_item = MenuItem::new("Show Yaperino", true, None);
        let quit_item = MenuItem::new("Quit", true, None);
        menu.append(&show_item).ok()?;
        menu.append(&quit_item).ok()?;
        let (show_id, quit_id) = (show_item.id().clone(), quit_item.id().clone());
        let icon = tray_icon::Icon::from_rgba(crate::logo::rgba(32), 32, 32).ok()?;
        let tray = TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip("Yaperino")
            .with_menu(Box::new(menu))
            .build()
            .ok()?;
        let c = ctx.clone();
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = e
            {
                show(hwnd);
                c.request_repaint();
            }
        }));
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            if e.id == show_id {
                show(hwnd);
                ctx.request_repaint();
            } else if e.id == quit_id {
                QUIT.store(true, Ordering::Relaxed);
                show(hwnd);
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                ctx.request_repaint();
            }
        }));
        Some(Tray { _icon: tray })
    }
    #[cfg(not(windows))]
    {
        let _ = (hwnd, ctx);
        None
    }
}
