//! Puts the Yaperino logo and name into the Windows program file, so Start, the
//! taskbar, Task Manager and the Apps list show them. The logo is drawn by
//! src/logo.rs (no image files).

#[path = "src/logo.rs"]
mod logo;

fn main() {
    println!("cargo:rerun-if-changed=src/logo.rs");
    println!("cargo:rerun-if-changed=build.rs");
    // the Twitch app's Client ID: from the environment, else a local file that is kept out of
    // git (.gitignore), so a published copy carries no one's app id
    println!("cargo:rerun-if-env-changed=YAPERINO_TWITCH_CLIENT_ID");
    println!("cargo:rerun-if-changed=twitch_client_id.txt");
    let id = std::env::var("YAPERINO_TWITCH_CLIENT_ID")
        .ok()
        .or_else(|| std::fs::read_to_string("twitch_client_id.txt").ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric()));
    if let Some(id) = id {
        println!("cargo:rustc-env=YAPERINO_TWITCH_CLIENT_ID={id}");
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let ico = out.join("yaperino.ico");
    if let Err(e) = std::fs::write(&ico, logo::ico(&logo::ICO_SIZES)) {
        println!("cargo:warning=could not write the icon: {e}");
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon(&ico.to_string_lossy());
    res.set("ProductName", "Yaperino");
    res.set("FileDescription", "Yaperino");
    res.set("CompanyName", "Yaperino");
    res.set("OriginalFilename", "Yaperino.exe");
    res.set("InternalName", "Yaperino");
    if let Err(e) = res.compile() {
        // the program still builds, only without its icon
        println!("cargo:warning=no icon in the program file: {e}");
    }
}
