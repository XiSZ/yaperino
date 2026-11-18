//! Link previews (Chatterino's link info): hovering a link in chat shows the page's title,
//! description and picture, or the picture itself for an image link. Yaperino asks the
//! link's own website (nothing goes to any other service), in the background, once per link
//! and session. Settings > General > Links turns it off.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

/// What a link shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Preview {
    pub title: String,
    pub description: String,
    /// A picture to show (the page's preview image, or the link itself for images).
    pub image: Option<String>,
    /// The website (host name).
    pub site: String,
}

#[derive(Clone)]
enum State {
    Loading,
    Done(Preview),
    Failed,
}

fn cache() -> &'static Mutex<HashMap<String, State>> {
    static C: OnceLock<Mutex<HashMap<String, State>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// A link's preview: Some(None) while it loads, None when there's none; the first call
/// starts loading it.
fn get(url: &str, ctx: &egui::Context) -> Option<Option<Preview>> {
    let mut c = cache().lock().ok()?;
    match c.get(url) {
        Some(State::Done(p)) => return Some(Some(p.clone())),
        Some(State::Loading) => return Some(None),
        Some(State::Failed) => return None,
        None => {}
    }
    c.insert(url.to_string(), State::Loading);
    let (u, ctx) = (url.to_string(), ctx.clone());
    std::thread::spawn(move || {
        let s = match fetch(&u) {
            Some(p) => State::Done(p),
            None => State::Failed,
        };
        if let Ok(mut c) = cache().lock() {
            c.insert(u, s);
        }
        ctx.request_repaint();
    });
    Some(None)
}

/// The tooltip for a hovered link.
pub fn ui(ui: &mut egui::Ui, url: &str) {
    ui.set_max_width(340.0);
    let host = host_of(url);
    match get(url, ui.ctx()) {
        Some(Some(p)) => {
            if let Some(img) = &p.image {
                ui.add(
                    egui::Image::new(img.as_str())
                        .max_size(egui::Vec2::new(320.0, 180.0))
                        .corner_radius(6),
                );
            }
            if !p.title.is_empty() {
                ui.label(egui::RichText::new(&p.title).strong());
            }
            if !p.description.is_empty() {
                ui.label(egui::RichText::new(&p.description).small());
            }
            ui.weak(if p.site.is_empty() { host } else { p.site });
        }
        Some(None) => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.weak(format!("Loading a preview of {host}"));
            });
        }
        None => {
            ui.weak(url);
        }
    }
    ui.weak("Click to open in your browser");
}

pub fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    rest.split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .trim_start_matches("www.")
        .to_string()
}

/// Asks the website (a few seconds at most, the first 512 KB of a page).
fn fetch(url: &str) -> Option<Preview> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(6)))
        .build()
        .into();
    let mut r = agent
        .get(url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Yaperino link preview",
        )
        .header("Accept-Language", "en")
        .call()
        .ok()?;
    let kind = r
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();
    if kind.starts_with("image/") {
        return Some(Preview {
            title: String::new(),
            description: String::new(),
            image: Some(url.to_string()),
            site: host_of(url),
        });
    }
    if !kind.contains("html") {
        return None;
    }
    let html = r
        .body_mut()
        .with_config()
        .limit(512 * 1024)
        .read_to_string()
        .ok()?;
    let p = from_html(&html, url);
    (!p.title.is_empty() || p.image.is_some()).then_some(p)
}

/// The page's Open Graph / Twitter card tags, else its <title>.
pub fn from_html(html: &str, url: &str) -> Preview {
    let mut meta: HashMap<String, String> = HashMap::new();
    let lower = html.to_lowercase();
    let mut at = 0;
    while let Some(i) = lower[at..].find("<meta") {
        let start = at + i;
        let end = lower[start..].find('>').map_or(lower.len(), |e| start + e);
        let tag = &html[start..end];
        let key = attr(tag, "property").or_else(|| attr(tag, "name"));
        if let (Some(k), Some(v)) = (key, attr(tag, "content")) {
            meta.entry(k.to_lowercase()).or_insert(v);
        }
        at = end;
    }
    let pick = |keys: &[&str]| {
        keys.iter()
            .find_map(|k| meta.get(*k).filter(|v| !v.trim().is_empty()).cloned())
    };
    let title = pick(&["og:title", "twitter:title"]).or_else(|| {
        let s = lower.find("<title")?;
        let s = s + lower[s..].find('>')? + 1;
        let e = s + lower[s..].find("</title")?;
        Some(html[s..e].to_string())
    });
    let image = pick(&[
        "og:image",
        "og:image:url",
        "twitter:image",
        "twitter:image:src",
    ])
    .map(|i| absolute(&i, url));
    Preview {
        title: clean(&title.unwrap_or_default(), 120),
        description: clean(
            &pick(&["og:description", "twitter:description", "description"]).unwrap_or_default(),
            220,
        ),
        image,
        site: pick(&["og:site_name"])
            .map(|s| clean(&s, 60))
            .unwrap_or_else(|| host_of(url)),
    }
}

/// An attribute's value in a tag (quoted with " or ').
fn attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_lowercase();
    let mut from = 0;
    while let Some(i) = lower[from..].find(name) {
        let s = from + i;
        from = s + name.len();
        // a whole attribute name, followed by =
        let before_ok = s == 0 || lower.as_bytes()[s - 1].is_ascii_whitespace();
        let rest = lower[from..].trim_start();
        if !before_ok || !rest.starts_with('=') {
            continue;
        }
        let v = tag[tag.len() - rest.len() + 1..].trim_start();
        let q = v.chars().next()?;
        if q == '"' || q == '\'' {
            let inner = &v[1..];
            return inner.find(q).map(|e| inner[..e].to_string());
        }
        return Some(v.split_whitespace().next().unwrap_or("").to_string());
    }
    None
}

/// HTML entities decoded, whitespace squashed, cut to `max` characters.
fn clean(s: &str, max: usize) -> String {
    let s = s
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ");
    let s: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() > max {
        s.chars().take(max - 1).chain(['…']).collect()
    } else {
        s
    }
}

/// A picture address relative to the page made absolute.
fn absolute(img: &str, page: &str) -> String {
    let img = img.replace("&amp;", "&");
    if img.starts_with("http://") || img.starts_with("https://") {
        return img;
    }
    let scheme = if page.starts_with("http://") {
        "http"
    } else {
        "https"
    };
    if let Some(rest) = img.strip_prefix("//") {
        return format!("{scheme}://{rest}");
    }
    let origin = format!(
        "{scheme}://{}",
        page.split_once("://")
            .map_or(page, |(_, r)| r)
            .split('/')
            .next()
            .unwrap_or("")
    );
    if img.starts_with('/') {
        format!("{origin}{img}")
    } else {
        format!("{origin}/{img}")
    }
}

/// A chat word as a link to open: http(s)://... or www...., without the punctuation after
/// it ("look: https://x.y/z." -> "https://x.y/z").
pub fn link_of(word: &str) -> Option<String> {
    let w = word.trim_end_matches(['.', ',', '!', '?', ')', ']', ';', ':', '"', '\'']);
    let w = w.trim_start_matches(['(', '[', '"', '\'']);
    let url = if w.starts_with("https://") || w.starts_with("http://") {
        w.to_string()
    } else if w.starts_with("www.") && w.len() > 6 {
        format!("https://{w}")
    } else {
        return None;
    };
    let host = host_of(&url);
    (host.contains('.') && !host.starts_with('.') && !host.ends_with('.')).then_some(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_are_found_in_chat_words() {
        assert_eq!(
            link_of("https://example.com/a?b=1"),
            Some("https://example.com/a?b=1".into())
        );
        assert_eq!(
            link_of("(https://example.com)."),
            Some("https://example.com".into())
        );
        assert_eq!(
            link_of("www.example.com"),
            Some("https://www.example.com".into())
        );
        assert_eq!(link_of("http://localhost"), None);
        assert_eq!(link_of("example.com"), None);
        assert_eq!(link_of("hello"), None);
        assert_eq!(host_of("https://www.youtube.com/watch?v=1"), "youtube.com");
    }

    #[test]
    fn page_cards_are_read() {
        let html = r#"<html><head><title>Fallback</title>
            <meta property="og:title" content="A &amp; B">
            <meta name='description' content='Plain description'>
            <meta property="og:image" content="/img/card.png" />
            <meta property="og:site_name" content="Example">
            </head></html>"#;
        let p = from_html(html, "https://example.com/page");
        assert_eq!(p.title, "A & B");
        assert_eq!(p.description, "Plain description");
        assert_eq!(p.image.as_deref(), Some("https://example.com/img/card.png"));
        assert_eq!(p.site, "Example");
        let plain = from_html("<title>\n  Just a title </title>", "https://x.org");
        assert_eq!((plain.title.as_str(), plain.image), ("Just a title", None));
        assert_eq!(plain.site, "x.org");
    }
}
