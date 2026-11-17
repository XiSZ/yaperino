//! Linux only: the Kick / YouTube sign-in window as its own small program, so that only it
//! needs WebKitGTK and Yaperino itself starts on any Linux. Yaperino runs it as
//! `yaperino-login --web-login kick|youtube` and reads the result from its output, exactly
//! like the `--web-login` start of the main program on Windows and macOS (see weblogin.rs).
//!
//! Built with `cargo build --release --features linux-login --bin yaperino-login`.

/// The same name as the app's, so the sign-in profile lands in Yaperino's folder.
pub const APP_ID: &str = "Yaperino";

#[allow(dead_code)]
#[path = "paths.rs"]
mod paths;
#[allow(dead_code)]
#[path = "weblogin.rs"]
mod weblogin;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.iter().position(|a| a == "--web-login") {
        Some(i) => std::process::exit(weblogin::run(
            args.get(i + 1).map(String::as_str).unwrap_or(""),
        )),
        None => {
            eprintln!("yaperino-login is started by Yaperino for signing in to Kick or YouTube");
            std::process::exit(2);
        }
    }
}
