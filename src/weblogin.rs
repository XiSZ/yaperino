//! Sign-in without a developer app: the site's own login page in a small window
//! (Microsoft Edge WebView2), the way desktop chat apps such as KickTalk sign in.
//!
//! The window runs as a separate start of this program (`live-streams.exe --web-login kick`
//! or `youtube`), because a program can only run one window loop. When the site has
//! signed the person in, its cookies and the browser's user agent go back to the app
//! over a pipe and the window closes by itself. Nothing is shown or logged.
//! Each site keeps its own browser profile under %LOCALAPPDATA%\Yaperino\WebView2\<site>
//! (outside Documents, where Windows' ransomware protection would block it).

use std::path::PathBuf;
#[cfg(not(target_os = "linux"))]
use std::sync::{Arc, Mutex};
#[cfg(not(target_os = "linux"))]
use std::time::{Duration, Instant};

/// The line the login window writes on its pipe, followed by the session as JSON.
const MARK: &str = "YAPERINO-SESSION ";

/// A site's sign-in: its cookies and the user agent of the browser that made them
/// (Kick's bot protection expects the same browser on later requests).
#[derive(Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct WebSession {
    pub cookies: Vec<(String, String)>,
    pub user_agent: String,
}

impl std::fmt::Debug for WebSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // never the values
        f.debug_struct("WebSession")
            .field("cookies", &self.cookies.len())
            .finish()
    }
}

impl WebSession {
    pub fn is_empty(&self) -> bool {
        self.cookies.is_empty()
    }
    pub fn get(&self, name: &str) -> Option<&str> {
        self.cookies
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
            .filter(|v| !v.is_empty())
    }
    /// The `Cookie:` header value.
    pub fn header(&self) -> String {
        self.cookies
            .iter()
            .map(|(n, v)| format!("{n}={v}"))
            .collect::<Vec<_>>()
            .join("; ")
    }
}

/// %-decoding (cookie values such as Kick's `session_token` are URL-encoded).
pub fn url_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(v) = std::str::from_utf8(&b[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg_attr(target_os = "linux", allow(dead_code))]
struct Site {
    title: &'static str,
    start: &'static str,
    cookie_url: &'static str,
    /// Signed in: these cookies are all there.
    needs: &'static [&'static str],
}

#[cfg_attr(target_os = "linux", allow(dead_code))]
fn site(name: &str) -> Option<Site> {
    match name {
        "kick" => Some(Site {
            title: "Sign in to Kick - Yaperino (this window closes by itself)",
            start: "https://kick.com/",
            cookie_url: "https://kick.com/",
            needs: &["session_token"],
        }),
        "youtube" => Some(Site {
            title: "Sign in to YouTube - Yaperino (this window closes by itself)",
            start: "https://accounts.google.com/ServiceLogin?service=youtube&passive=true&continue=https%3A%2F%2Fwww.youtube.com%2Fsignin%3Faction_handle_signin%3Dtrue%26app%3Ddesktop%26next%3Dhttps%253A%252F%252Fwww.youtube.com%252F",
            cookie_url: "https://www.youtube.com/",
            needs: &["SAPISID", "LOGIN_INFO"],
        }),
        _ => None,
    }
}

/// Where a site's browser profile lives.
pub fn profile_dir(site: &str) -> Option<PathBuf> {
    let folder = if cfg!(windows) { "WebView2" } else { "WebView" };
    crate::paths::local_dir().map(|d| d.join(folder).join(site))
}

/// Sign out: forget the site's browser profile, so the next sign-in asks again.
pub fn forget(site: &str) {
    if let Some(d) = profile_dir(site) {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// In the app: opens the login window and waits until it closes.
pub fn sign_in(site: &str) -> Result<WebSession, String> {
    if cfg!(target_os = "linux") {
        return Err(
            "Kick and YouTube sign-in isn't available on Linux yet. Chat can still be read.".into(),
        );
    }
    use std::io::Read;
    use std::process::{Command, Stdio};
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut child = Command::new(exe)
        .args(["--web-login", site])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("could not open the sign-in window: {e}"))?;
    let mut out = String::new();
    if let Some(mut o) = child.stdout.take() {
        let _ = o.read_to_string(&mut out);
    }
    let _ = child.wait();
    out.lines()
        .find_map(|l| l.strip_prefix(MARK))
        .and_then(|j| serde_json::from_str::<WebSession>(j).ok())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "The sign-in window was closed before signing in".to_string())
}

// ---------------------------------------------------------------- the login window
// Windows (WebView2) and macOS (WKWebView). Linux would need WebKitGTK and a GTK window,
// which isn't done yet: there `run` only reports that.

#[cfg(target_os = "linux")]
pub fn run(_site_name: &str) -> i32 {
    eprintln!("Kick and YouTube sign-in isn't available on Linux yet");
    5
}

#[cfg(not(target_os = "linux"))]
struct Login {
    site: Site,
    context: wry::WebContext,
    window: Option<winit::window::Window>,
    view: Option<wry::WebView>,
    last: Instant,
    user_agent: Arc<Mutex<Option<String>>>,
    found: Option<(Vec<(String, String)>, Instant)>,
    result: Option<WebSession>,
    failed: bool,
}

#[cfg(not(target_os = "linux"))]
impl winit::application::ApplicationHandler for Login {
    fn resumed(&mut self, el: &winit::event_loop::ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = winit::window::Window::default_attributes()
            .with_title(self.site.title)
            .with_inner_size(winit::dpi::LogicalSize::new(560.0, 780.0));
        let Ok(window) = el.create_window(attrs) else {
            self.failed = true;
            el.exit();
            return;
        };
        let ua = self.user_agent.clone();
        let view = wry::WebViewBuilder::new_with_web_context(&mut self.context)
            .with_url(self.site.start)
            .with_ipc_handler(move |req: wry::http::Request<String>| {
                if let Ok(mut u) = ua.lock() {
                    *u = Some(req.body().clone());
                }
            })
            .build(&window);
        match view {
            Ok(v) => self.view = Some(v),
            Err(_) => {
                self.failed = true;
                el.exit();
            }
        }
        self.window = Some(window);
    }

    fn window_event(
        &mut self,
        el: &winit::event_loop::ActiveEventLoop,
        _id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        match event {
            winit::event::WindowEvent::CloseRequested => el.exit(),
            winit::event::WindowEvent::Resized(size) => {
                if let Some(v) = &self.view {
                    let _ = v.set_bounds(wry::Rect {
                        position: wry::dpi::PhysicalPosition::new(0, 0).into(),
                        size: wry::dpi::PhysicalSize::new(size.width, size.height).into(),
                    });
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, el: &winit::event_loop::ActiveEventLoop) {
        el.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(400),
        ));
        if self.last.elapsed() < Duration::from_millis(800) {
            return;
        }
        self.last = Instant::now();
        let Some(view) = &self.view else { return };
        match &self.found {
            None => {
                let Ok(cookies) = view.cookies_for_url(self.site.cookie_url) else {
                    return;
                };
                let list: Vec<(String, String)> = cookies
                    .iter()
                    .map(|c| (c.name().to_string(), c.value().to_string()))
                    .collect();
                let signed_in = self
                    .site
                    .needs
                    .iter()
                    .all(|n| list.iter().any(|(k, v)| k == n && !v.is_empty()));
                if signed_in {
                    // also the browser's user agent, for later requests with these cookies
                    let _ = view.evaluate_script("window.ipc.postMessage(navigator.userAgent)");
                    self.found = Some((list, Instant::now()));
                }
            }
            Some((list, at)) => {
                let ua = self.user_agent.lock().ok().and_then(|u| u.clone());
                if ua.is_some() || at.elapsed() > Duration::from_secs(3) {
                    self.result = Some(WebSession {
                        cookies: list.clone(),
                        user_agent: ua.unwrap_or_default(),
                    });
                    el.exit();
                }
            }
        }
    }
}

/// `--web-login <site>`: runs the login window; returns the process exit code.
#[cfg(not(target_os = "linux"))]
pub fn run(site_name: &str) -> i32 {
    let Some(site) = site(site_name) else {
        return 2;
    };
    let context = wry::WebContext::new(profile_dir(site_name));
    let Ok(event_loop) = winit::event_loop::EventLoop::new() else {
        return 3;
    };
    let mut app = Login {
        site,
        context,
        window: None,
        view: None,
        last: Instant::now(),
        user_agent: Arc::new(Mutex::new(None)),
        found: None,
        result: None,
        failed: false,
    };
    let _ = event_loop.run_app(&mut app);
    if let Some(s) = app.result.take() {
        if let Ok(j) = serde_json::to_string(&s) {
            use std::io::Write;
            let mut o = std::io::stdout();
            let _ = writeln!(o, "{MARK}{j}");
            let _ = o.flush();
            return 0;
        }
    }
    if app.failed {
        4
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_helpers() {
        let s = WebSession {
            cookies: vec![
                ("a".into(), "1".into()),
                ("session_token".into(), "12%7Cab".into()),
            ],
            user_agent: String::new(),
        };
        assert_eq!(s.header(), "a=1; session_token=12%7Cab");
        assert_eq!(url_decode(s.get("session_token").unwrap()), "12|ab");
        assert_eq!(url_decode("100%"), "100%");
        assert!(s.get("missing").is_none());
        assert!(!format!("{s:?}").contains("12%7Cab"));
    }
}
