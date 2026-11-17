//! Moltorino's message translation: message menu > Translate puts the translation under the
//! message, and the split menu's "Auto-translate this chat" translates a chat's new messages
//! as they arrive. Google's keyless web translator by default, or DeepL with the user's own
//! key. Only the message text is sent, and only when translating. Requests go one at a time,
//! at most one a second; translations are kept per message for the session. A child of `app`.

use super::*;

/// Languages to translate to (Google's codes), with their names. Also names the language a
/// message was detected in.
pub const LANGUAGES: [(&str, &str); 36] = [
    ("en", "English"),
    ("de", "German"),
    ("es", "Spanish"),
    ("fr", "French"),
    ("it", "Italian"),
    ("pt", "Portuguese"),
    ("nl", "Dutch"),
    ("pl", "Polish"),
    ("ru", "Russian"),
    ("uk", "Ukrainian"),
    ("tr", "Turkish"),
    ("sv", "Swedish"),
    ("da", "Danish"),
    ("no", "Norwegian"),
    ("fi", "Finnish"),
    ("cs", "Czech"),
    ("sk", "Slovak"),
    ("hu", "Hungarian"),
    ("ro", "Romanian"),
    ("bg", "Bulgarian"),
    ("el", "Greek"),
    ("lt", "Lithuanian"),
    ("lv", "Latvian"),
    ("et", "Estonian"),
    ("sl", "Slovenian"),
    ("ja", "Japanese"),
    ("ko", "Korean"),
    ("zh-CN", "Chinese"),
    ("ar", "Arabic"),
    ("he", "Hebrew"),
    ("hi", "Hindi"),
    ("id", "Indonesian"),
    ("ms", "Malay"),
    ("th", "Thai"),
    ("vi", "Vietnamese"),
    ("tl", "Filipino"),
];

/// At most this many messages wait for auto-translation; in a busy chat older ones are
/// skipped instead of falling minutes behind.
const BACKLOG: usize = 15;

/// A finished translation and the language the message was in (as the service names it).
#[derive(Clone, Debug, PartialEq)]
pub struct Translation {
    pub text: String,
    pub source: String,
}

/// One message to translate: its tab, the message's key (see `line_key`), the text.
#[derive(Clone, Debug)]
pub struct Request {
    pub group: String,
    pub key: String,
    pub text: String,
    /// Asked for with message menu > Translate (not auto-translation).
    pub manual: bool,
    /// The language it goes to, as asked.
    pub target: String,
}

/// The translation queue and the session's translations (one per `LiveApp`).
#[derive(Default)]
pub struct Translator {
    /// "<target>|<message key>" -> translation.
    cache: HashMap<String, Translation>,
    queue: VecDeque<Request>,
    busy: bool,
    last_sent: Option<Instant>,
    /// Tabs whose auto-translation failed: said once, until one works again.
    failed: HashSet<String>,
    shot_done: bool,
}

/// The base language of a code: "DE" -> "de", "zh-CN" -> "zh", "EN-US" -> "en".
fn base(code: &str) -> String {
    let b = code.split(['-', '_']).next().unwrap_or("").to_lowercase();
    // Google still says "iw" for Hebrew; DeepL says "nb" for Norwegian
    match b.as_str() {
        "iw" => "he".into(),
        "nb" | "nn" => "no".into(),
        _ => b,
    }
}

/// Whether two language codes mean the same language.
pub fn same_language(a: &str, b: &str) -> bool {
    !a.is_empty() && base(a) == base(b)
}

/// "German" for "de" / "DE"; the code itself for one not in the list.
pub fn language_name(code: &str) -> String {
    let b = base(code);
    LANGUAGES
        .iter()
        .find(|(c, _)| base(c) == b)
        .map(|(_, n)| n.to_string())
        .unwrap_or_else(|| code.to_string())
}

/// DeepL's target language for one of `LANGUAGES` (DeepL wants a variant for English,
/// Portuguese and Chinese).
fn deepl_target(code: &str) -> String {
    match base(code).as_str() {
        "en" => "EN-US".into(),
        "pt" => "PT-BR".into(),
        "zh" => "ZH-HANS".into(),
        "no" => "NB".into(),
        b => b.to_uppercase(),
    }
}

/// DeepL's free API for free keys (they end in ":fx"), the paid one otherwise.
fn deepl_url(key: &str) -> &'static str {
    if key.trim().ends_with(":fx") {
        "https://api-free.deepl.com/v2/translate"
    } else {
        "https://api.deepl.com/v2/translate"
    }
}

/// Google's answer: [[["translated part","source part",...],...], null, "de", ...].
pub fn parse_google(body: &str) -> Result<Translation, String> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|_| "Google sent something unexpected".to_string())?;
    let parts = v
        .get(0)
        .and_then(|p| p.as_array())
        .ok_or("Google sent no translation")?;
    let text: String = parts
        .iter()
        .filter_map(|p| p.get(0).and_then(|t| t.as_str()))
        .collect();
    if text.trim().is_empty() {
        return Err("Google sent no translation".into());
    }
    let source = v
        .get(2)
        .and_then(|s| s.as_str())
        .unwrap_or_default()
        .to_string();
    Ok(Translation {
        text: text.trim().to_string(),
        source,
    })
}

/// DeepL's answer: {"translations":[{"detected_source_language":"DE","text":"..."}]}.
pub fn parse_deepl(body: &str) -> Result<Translation, String> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|_| "DeepL sent something unexpected".to_string())?;
    let t = v
        .get("translations")
        .and_then(|t| t.get(0))
        .ok_or("DeepL sent no translation")?;
    let s = |k: &str| {
        t.get(k)
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string()
    };
    Ok(Translation {
        text: s("text").trim().to_string(),
        source: s("detected_source_language"),
    })
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .http_status_as_error(false)
        .build()
        .into()
}

fn google(text: &str, target: &str) -> Result<Translation, String> {
    let mut r = agent()
        .get("https://translate.googleapis.com/translate_a/single")
        .query("client", "gtx")
        .query("sl", "auto")
        .query("tl", target)
        .query("dt", "t")
        .query("q", text)
        .call()
        .map_err(|e| format!("Google: {e}"))?;
    let body = r.body_mut().read_to_string().unwrap_or_default();
    match r.status().as_u16() {
        200 => parse_google(&body),
        429 => Err("Google says too many translations; try again in a while".into()),
        code => Err(format!("Google answered {code}")),
    }
}

fn deepl(text: &str, target: &str, key: &str) -> Result<Translation, String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("add your DeepL key in Settings > General > Translation".into());
    }
    let body = serde_json::json!({ "text": [text], "target_lang": deepl_target(target) });
    let mut r = agent()
        .post(deepl_url(key))
        .header("Authorization", &format!("DeepL-Auth-Key {key}"))
        .header("Content-Type", "application/json")
        .send(body.to_string())
        // the error never holds the key (it's only in a header)
        .map_err(|e| format!("DeepL: {e}"))?;
    let answer = r.body_mut().read_to_string().unwrap_or_default();
    match r.status().as_u16() {
        200 => parse_deepl(&answer),
        403 => Err("DeepL refused the key (Settings > General > Translation)".into()),
        456 => Err("this month's DeepL characters are used up".into()),
        429 => Err("DeepL says too many translations; try again in a while".into()),
        code => Err(format!("DeepL answered {code}")),
    }
}

/// Translates with the chosen service ("DeepL" uses the key, anything else Google).
pub fn translate(
    text: &str,
    target: &str,
    provider: &str,
    deepl_key: &str,
) -> Result<Translation, String> {
    if provider == "DeepL" {
        deepl(text, target, deepl_key)
    } else {
        google(text, target)
    }
}

/// What identifies a message for the cache and for finding it again: Twitch's message id,
/// or (Kick, YouTube, TikTok) its platform, time, author and text.
pub fn line_key(line: &ChatLine) -> Option<String> {
    match line {
        ChatLine::Msg {
            msg_id,
            platform,
            time,
            name,
            text,
            ..
        } => Some(if msg_id.is_empty() {
            format!("{}|{time}|{name}|{text}", platform.label())
        } else {
            msg_id.clone()
        }),
        ChatLine::System(_) => None,
    }
}

/// The words of a message, without its emotes (their names would come back "translated").
fn words_of(line: &ChatLine) -> String {
    let ChatLine::Msg { text, segs, .. } = line else {
        return String::new();
    };
    let raw = if segs.is_empty() {
        text.clone()
    } else {
        segs.iter()
            .filter_map(|s| match s {
                emotes::Seg::Text(t) => Some(t.as_str()),
                emotes::Seg::Emote(_) => None,
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Text worth translating: it has letters (not only numbers, links or punctuation).
fn has_words(text: &str) -> bool {
    text.split_whitespace()
        .filter(|w| !w.starts_with("http://") && !w.starts_with("https://") && !w.starts_with('@'))
        .any(|w| w.chars().any(char::is_alphabetic))
}

impl LiveApp {
    /// Message menu > Translate (manual) or auto-translation: queue the message, or show the
    /// translation at once when this session already has it.
    pub(super) fn translate_line(&mut self, group: &str, line: &ChatLine, manual: bool) {
        let Some(key) = line_key(line) else {
            return;
        };
        let text = words_of(line);
        if !has_words(&text) {
            if manual {
                self.insert_after(group, &key, "Nothing to translate in this message".into());
            }
            return;
        }
        let target = self.settings.translate_to.clone();
        let req = Request {
            group: group.to_string(),
            key,
            text,
            manual,
            target,
        };
        let cached = self
            .translator
            .cache
            .get(&format!("{}|{}", req.target, req.key))
            .cloned();
        if let Some(t) = cached {
            self.show_translation(&req, &t);
            return;
        }
        let q = &mut self.translator.queue;
        if q.iter().any(|r| r.group == req.group && r.key == req.key) {
            return;
        }
        if manual {
            q.push_front(req);
        } else {
            q.push_back(req);
            // a busy chat: skip the oldest waiting messages
            while q.iter().filter(|r| !r.manual).count() > BACKLOG {
                if let Some(i) = q.iter().position(|r| !r.manual) {
                    q.remove(i);
                }
            }
        }
    }

    /// A new message arrived for channel `ch`: auto-translate it in the tabs that have
    /// "Auto-translate this chat" on (not own messages, emote-only ones or !commands).
    pub(super) fn auto_translate(&mut self, ch: &str, line: &ChatLine) {
        if self.settings.auto_translate.is_empty() {
            return;
        }
        let ChatLine::Msg {
            own: false, text, ..
        } = line
        else {
            return;
        };
        if text.trim_start().starts_with('!') {
            return;
        }
        let groups: Vec<String> = self
            .chats
            .iter()
            .filter(|t| t.login == ch || t.twitch.as_deref() == Some(ch))
            .filter(|t| self.settings.auto_translate.contains(&t.login))
            .map(|t| t.login.clone())
            .collect();
        for g in groups {
            // the emotes are only known once the line has its segments
            let mut l = line.clone();
            let channel = self
                .chats
                .iter()
                .find(|t| t.login == g)
                .and_then(|t| t.twitch.clone())
                .unwrap_or_else(|| g.clone());
            fill_segments(&mut l, &self.emotes, &channel);
            self.translate_line(&g, &l, false);
        }
    }

    /// Split menu > Auto-translate this chat: on or off for this tab.
    pub(super) fn toggle_auto_translate(&mut self, group: &str) {
        let list = &mut self.settings.auto_translate;
        let note = if let Some(i) = list.iter().position(|c| c == group) {
            list.remove(i);
            self.translator
                .queue
                .retain(|r| r.manual || r.group != group);
            "Auto-translate is off".to_string()
        } else {
            list.push(group.to_string());
            format!(
                "Auto-translate is on: new messages in other languages are translated to {} ({})",
                language_name(&self.settings.translate_to),
                if self.settings.translate_provider == "DeepL" {
                    "DeepL"
                } else {
                    "Google"
                }
            )
        };
        self.push_to(group, ChatLine::System(note));
    }

    /// Sends the next waiting message: one at a time, at most one a second (the window
    /// repaints every second, so the queue keeps moving).
    pub(super) fn pump_translations(&mut self) {
        self.translate_shot();
        let t = &mut self.translator;
        if t.busy
            || t.queue.is_empty()
            || t.last_sent
                .is_some_and(|l| l.elapsed() < Duration::from_secs(1))
        {
            return;
        }
        let Some(req) = t.queue.pop_front() else {
            return;
        };
        t.busy = true;
        t.last_sent = Some(Instant::now());
        let tx = self.side_tx.clone();
        if self.demo {
            // sample data never touches the network
            let r = Ok(Translation {
                text: "Hello everyone, greetings from Berlin! How long are you streaming today?"
                    .into(),
                source: "de".into(),
            });
            let _ = tx.send(SideMsg::Translated(req, r));
            return;
        }
        let provider = self.settings.translate_provider.clone();
        let key = self.settings.deepl_key.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let r = translate(&req.text, &req.target, &provider, &key);
            let _ = tx.send(SideMsg::Translated(req, r));
            ctx.request_repaint();
        });
    }

    /// A translation came back (or failed).
    pub(super) fn translation_done(&mut self, req: Request, r: Result<Translation, String>) {
        self.translator.busy = false;
        match r {
            Ok(t) => {
                self.translator.failed.remove(&req.group);
                self.translator
                    .cache
                    .insert(format!("{}|{}", req.target, req.key), t.clone());
                self.show_translation(&req, &t);
            }
            Err(e) => {
                if req.manual {
                    self.insert_after(&req.group, &req.key, format!("Translation failed: {e}"));
                } else if self.translator.failed.insert(req.group.clone()) {
                    self.insert_after(
                        &req.group,
                        &req.key,
                        format!("Auto-translate failed: {e} (the split menu turns it off)"),
                    );
                }
            }
        }
    }

    /// The translation under its message; a message already in the target language only
    /// says so when asked for.
    fn show_translation(&mut self, req: &Request, t: &Translation) {
        let note = if same_language(&t.source, &req.target) || t.text.trim() == req.text.trim() {
            if !req.manual {
                return;
            }
            format!("Already in {}", language_name(&req.target))
        } else if t.source.is_empty() {
            format!("Translated: {}", t.text)
        } else {
            format!("Translated from {}: {}", language_name(&t.source), t.text)
        };
        self.insert_after(&req.group, &req.key, note);
    }

    /// Puts a note right under a message of a tab (not again when it's already there).
    /// A message that has scrolled out of the chat gets nothing.
    fn insert_after(&mut self, group: &str, key: &str, note: String) {
        let Some(tab) = self.chats.iter_mut().find(|t| t.login == group) else {
            return;
        };
        let Some(i) = tab
            .lines
            .iter()
            .rposition(|l| line_key(l).as_deref() == Some(key))
        else {
            return;
        };
        if matches!(tab.lines.get(i + 1), Some(ChatLine::System(s)) if *s == note) {
            return;
        }
        tab.lines.insert(i + 1, ChatLine::System(note));
        let limit = MESSAGE_LIMIT
            .load(std::sync::atomic::Ordering::Relaxed)
            .clamp(50, 5000);
        while tab.lines.len() > limit {
            tab.lines.pop_front();
        }
    }

    /// Screenshot test only (LIVE_STREAMS_SHOT_TRANSLATE): a German sample message,
    /// translated with sample data, and auto-translate on for that chat.
    fn translate_shot(&mut self) {
        if self.translator.shot_done
            || self.shot.is_none()
            || std::env::var("LIVE_STREAMS_SHOT_TRANSLATE").is_err()
        {
            return;
        }
        if std::env::var("LIVE_STREAMS_SHOT_SETTINGS").is_ok() {
            // Settings > General scrolled to Translation, with DeepL picked
            self.translator.shot_done = true;
            self.settings_ui.scroll_to = Some("Translation");
            self.settings.translate_provider = "DeepL".into();
            self.settings.deepl_key = "sample-key:fx".into();
            self.settings.auto_translate = vec!["samplechannel".into()];
            return;
        }
        let Some(tab) = self.chats.first() else {
            return;
        };
        self.translator.shot_done = true;
        let group = tab.login.clone();
        let line = ChatLine::Msg {
            time: "18:05".into(),
            name: "berlin_viewer".into(),
            color: Some([218, 165, 32]),
            text: "Hallo zusammen, Grüße aus Berlin! Wie lange streamst du heute?".into(),
            action: false,
            own: false,
            emotes: Vec::new(),
            badges: Vec::new(),
            segs: Vec::new(),
            platform: Platform::Twitch,
            msg_id: "demo-translate".into(),
            user_id: String::new(),
            reply: None,
        };
        self.push_to(&group, line.clone());
        // every split shows "Auto-translate this chat" ticked in its menu
        self.settings.auto_translate = self.chats.iter().map(|t| t.login.clone()).collect();
        self.translate_line(&group, &line, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_googles_answer() {
        let body = r#"[[["Hello everyone, ","Hallo zusammen, ",null,null,10],["how are you?","wie geht's?",null,null,10]],null,"de",null,null,null,1,[],[["de"],null,[1],["de"]]]"#;
        let t = parse_google(body).unwrap();
        assert_eq!(t.text, "Hello everyone, how are you?");
        assert_eq!(t.source, "de");
        assert_eq!(language_name(&t.source), "German");
        assert!(parse_google("<html>").is_err());
        assert!(parse_google("[[],null,\"en\"]").is_err());
    }

    #[test]
    fn reads_deepls_answer() {
        let body = r#"{"translations":[{"detected_source_language":"DE","text":"Good morning"}]}"#;
        let t = parse_deepl(body).unwrap();
        assert_eq!(t.text, "Good morning");
        assert!(!same_language(&t.source, "en"));
        assert!(same_language(&t.source, "de"));
        assert!(parse_deepl(r#"{"message":"Wrong endpoint"}"#).is_err());
    }

    #[test]
    fn languages_and_endpoints() {
        assert!(same_language("EN", "en"));
        assert!(same_language("zh-CN", "ZH"));
        assert!(same_language("iw", "he"));
        assert!(!same_language("", "en"));
        assert_eq!(language_name("ZH"), "Chinese");
        assert_eq!(language_name("xx"), "xx");
        assert_eq!(deepl_target("en"), "EN-US");
        assert_eq!(deepl_target("zh-CN"), "ZH-HANS");
        assert_eq!(deepl_target("de"), "DE");
        assert_eq!(
            deepl_url("abc:fx"),
            "https://api-free.deepl.com/v2/translate"
        );
        assert_eq!(deepl_url("abc"), "https://api.deepl.com/v2/translate");
    }

    #[test]
    fn what_gets_translated() {
        assert!(has_words("hola amigos"));
        assert!(!has_words("123 !!! https://example.com/x @someone"));
        let line = ChatLine::Msg {
            time: "12:00".into(),
            name: "viewer".into(),
            color: None,
            text: "hola Kappa amigos".into(),
            action: false,
            own: false,
            emotes: Vec::new(),
            badges: Vec::new(),
            segs: vec![
                emotes::Seg::Text("hola ".into()),
                emotes::Seg::Emote(emotes::Emote {
                    name: "Kappa".into(),
                    url: String::new(),
                    provider: emotes::Provider::Twitch,
                }),
                emotes::Seg::Text(" amigos".into()),
            ],
            platform: Platform::Kick,
            msg_id: String::new(),
            user_id: String::new(),
            reply: None,
        };
        assert_eq!(words_of(&line), "hola amigos");
        assert_eq!(
            line_key(&line).as_deref(),
            Some("Kick|12:00|viewer|hola Kappa amigos")
        );
        assert_eq!(line_key(&ChatLine::System("x".into())), None);
    }
}
