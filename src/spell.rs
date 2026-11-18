//! Spellcheck for the message box (Moltorino): misspelled words get a red line and a right
//! click offers suggestions, in the languages picked in Settings (a word is right when any
//! of them knows it). English is built in: the Hunspell dictionary from SCOWL
//! (wordlist.sourceforge.net, through github.com/wooorm/dictionaries; its licence is in
//! assets/dictionaries). Other languages are downloaded once from github.com/wooorm/dictionaries
//! when picked (each with its own open licence, saved next to it) and kept in
//! <Yaperino's folder>/Dictionaries. Checked with the spellbook crate.
//!
//! Chat isn't an essay, so a lot is left alone: emotes, chatters' names, own words added
//! with "Add to dictionary", @mentions, :emote, /commands, links, words with digits,
//! CamelCase (emote-like) words and common chat words. The word being typed is only checked
//! once it's finished.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Arc, Mutex, OnceLock};

static EN_AFF: &str = include_str!("../assets/dictionaries/en_US.aff");
static EN_DIC: &str = include_str!("../assets/dictionaries/en_US.dic");

/// The languages offered: (wooorm/dictionaries code, name). "en" is built in.
pub const LANGUAGES: &[(&str, &str)] = &[
    ("en", "English (US)"),
    ("en-GB", "English (UK)"),
    ("de", "German"),
    ("de-AT", "German (Austria)"),
    ("de-CH", "German (Switzerland)"),
    ("tr", "Turkish"),
    ("fr", "French"),
    ("es", "Spanish"),
    ("it", "Italian"),
    ("nl", "Dutch"),
    ("pt", "Portuguese (Brazil)"),
    ("pt-PT", "Portuguese (Portugal)"),
    ("pl", "Polish"),
    ("ru", "Russian"),
    ("uk", "Ukrainian"),
    ("sv", "Swedish"),
    ("da", "Danish"),
    ("nb", "Norwegian"),
    ("cs", "Czech"),
    ("sk", "Slovak"),
    ("ro", "Romanian"),
    ("hu", "Hungarian"),
    ("el", "Greek"),
    ("bg", "Bulgarian"),
    ("hr", "Croatian"),
    ("sl", "Slovenian"),
    ("lt", "Lithuanian"),
    ("lv", "Latvian"),
    ("et", "Estonian"),
    ("ca", "Catalan"),
    ("vi", "Vietnamese"),
    ("ko", "Korean"),
    ("he", "Hebrew"),
];

/// Where the other languages come from.
const SOURCE: &str = "https://raw.githubusercontent.com/wooorm/dictionaries/main/dictionaries/";

#[derive(Clone)]
enum Load {
    Loading,
    Ready(Arc<spellbook::Dictionary>),
    Failed(String),
}

fn loaded() -> &'static Mutex<HashMap<String, Load>> {
    static L: OnceLock<Mutex<HashMap<String, Load>>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The picked languages' dictionaries that are ready (None: none yet). The first call for a
/// language starts loading it in the background, so the message box never waits.
pub fn dictionaries(langs: &[String]) -> Option<Dicts> {
    let mut ready = Vec::new();
    let mut m = loaded().lock().ok()?;
    for code in langs {
        match m.get(code) {
            Some(Load::Ready(d)) => ready.push(d.clone()),
            Some(_) => {}
            None => {
                m.insert(code.clone(), Load::Loading);
                let c = code.clone();
                std::thread::spawn(move || {
                    let state = match load(&c) {
                        Ok(d) => Load::Ready(Arc::new(d)),
                        Err(e) => Load::Failed(e),
                    };
                    if let Ok(mut m) = loaded().lock() {
                        m.insert(c, state);
                    }
                });
            }
        }
    }
    (!ready.is_empty()).then_some(Dicts(ready))
}

/// A language's state for Settings: None = not used yet, Ok(false) = loading,
/// Ok(true) = ready, Err = why it couldn't be loaded.
pub fn status(code: &str) -> Option<Result<bool, String>> {
    match loaded().lock().ok()?.get(code)? {
        Load::Loading => Some(Ok(false)),
        Load::Ready(_) => Some(Ok(true)),
        Load::Failed(e) => Some(Err(e.clone())),
    }
}

/// A language's dictionary: built in (English), from Yaperino's folder, or downloaded there.
fn load(code: &str) -> Result<spellbook::Dictionary, String> {
    load_into(code, &crate::paths::app_dir_or_here().join("Dictionaries"))
}

/// A language's dictionary, kept in (or downloaded into) `dir`.
fn load_into(code: &str, dir: &std::path::Path) -> Result<spellbook::Dictionary, String> {
    if code == "en" {
        return spellbook::Dictionary::new(EN_AFF, EN_DIC).map_err(|e| e.to_string());
    }
    if !LANGUAGES.iter().any(|(c, _)| *c == code) {
        return Err(format!("unknown language {code}"));
    }
    let aff = dir.join(format!("{code}.aff"));
    let dic = dir.join(format!("{code}.dic"));
    if !(aff.exists() && dic.exists()) {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        for (file, path) in [("index.aff", &aff), ("index.dic", &dic)] {
            let body = download(&format!("{SOURCE}{code}/{file}"))?;
            // written whole or not at all
            let part = path.with_extension("part");
            std::fs::write(&part, body).map_err(|e| e.to_string())?;
            std::fs::rename(&part, path).map_err(|e| e.to_string())?;
        }
        if let Ok(l) = download(&format!("{SOURCE}{code}/license")) {
            let _ = std::fs::write(dir.join(format!("{code}-LICENSE.txt")), l);
        }
    }
    let read = |p: &std::path::Path| std::fs::read_to_string(p).map_err(|e| e.to_string());
    let (aff, dic) = tidy(&read(&aff)?, &read(&dic)?);
    spellbook::Dictionary::new(&aff, &dic).map_err(|e| e.to_string())
}

/// Directives whose value is one flag (shifted with the rest, see `tidy`).
const FLAG_DIRECTIVES: &[&str] = &[
    "COMPOUNDFLAG",
    "COMPOUNDBEGIN",
    "COMPOUNDMIDDLE",
    "COMPOUNDEND",
    "COMPOUNDLAST",
    "ONLYINCOMPOUND",
    "COMPOUNDPERMITFLAG",
    "COMPOUNDFORBIDFLAG",
    "NEEDAFFIX",
    "CIRCUMFIX",
    "FORBIDDENWORD",
    "KEEPCASE",
    "NOSUGGEST",
    "WARN",
    "SUBSTANDARD",
    "FORCEUCASE",
];

/// Some dictionaries need tidying before spellbook reads them (Hunspell itself is lenient):
/// numeric flags that start at 0 (Turkish) are moved up by one everywhere, and word-list
/// lines whose flags aren't numbers (Danish "A/S", where the slash is part of the word) are
/// left out.
fn tidy(aff: &str, dic: &str) -> (String, String) {
    let tokens = |l: &str| l.split_whitespace().map(String::from).collect::<Vec<_>>();
    let numeric = aff.lines().any(|l| tokens(l) == ["FLAG", "num"]);
    if !numeric {
        return (aff.to_string(), dic.to_string());
    }
    let zero = aff.lines().any(|l| {
        let t = tokens(l);
        matches!(t.first().map(String::as_str), Some("SFX" | "PFX"))
            && t.get(1).is_some_and(|f| f == "0")
    });
    let by = u32::from(zero);
    // "3,10" -> "4,11" (or the same when nothing moves); None when it isn't a flag list
    let shift = |flags: &str| -> Option<String> {
        flags
            .split(',')
            .map(|n| n.trim().parse::<u32>().ok().map(|v| (v + by).to_string()))
            .collect::<Option<Vec<_>>>()
            .map(|v| v.join(","))
    };
    let aff = if zero {
        aff.lines()
            .map(|l| {
                let mut t = tokens(l);
                match t.first().map(String::as_str) {
                    Some("SFX" | "PFX") if t.len() > 2 => {
                        if let Some(f) = shift(&t[1]) {
                            t[1] = f;
                        }
                        // the affix's own continuation flags: "ler/3,4"
                        if let Some((a, f)) = t.get(3).and_then(|x| x.split_once('/')) {
                            if let Some(f) = shift(f) {
                                t[3] = format!("{a}/{f}");
                            }
                        }
                        t.join(" ")
                    }
                    Some(d) if FLAG_DIRECTIVES.contains(&d) && t.len() == 2 => {
                        if let Some(f) = shift(&t[1]) {
                            t[1] = f;
                        }
                        t.join(" ")
                    }
                    _ => l.to_string(),
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        aff.to_string()
    };
    let mut out = String::with_capacity(dic.len());
    for (i, line) in dic.lines().enumerate() {
        if i > 0 {
            let (entry, rest) = line.split_at(line.find(char::is_whitespace).unwrap_or(line.len()));
            // the flags start after the last slash that isn't escaped (\/)
            let slash = entry
                .char_indices()
                .rev()
                .find(|(p, c)| *c == '/' && !entry[..*p].ends_with('\\'))
                .map(|(p, _)| p);
            if let Some(p) = slash {
                let Some(flags) = shift(&entry[p + 1..]) else {
                    continue;
                };
                out.push_str(&entry[..p]);
                out.push('/');
                out.push_str(&flags);
                out.push_str(rest);
                out.push('\n');
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    (aff, out)
}

fn download(url: &str) -> Result<String, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(60)))
        .build()
        .into();
    let mut r = agent
        .get(url)
        .header("User-Agent", "Yaperino spellcheck")
        .call()
        .map_err(|e| format!("download failed ({e})"))?;
    r.body_mut()
        .with_config()
        .limit(40 * 1024 * 1024)
        .read_to_string()
        .map_err(|e| format!("download failed ({e})"))
}

/// The dictionaries in use: a word is right when any of them knows it.
#[derive(Clone)]
pub struct Dicts(Vec<Arc<spellbook::Dictionary>>);

impl Dicts {
    fn check(&self, word: &str) -> bool {
        self.0.iter().any(|d| d.check(word))
    }
}

/// Chat words that aren't in a dictionary.
const CHAT_WORDS: &[&str] = &[
    "afk", "bc", "brb", "bruh", "btw", "cuz", "dm", "dms", "emote", "emotes", "ez", "fr", "gg",
    "ggs", "gl", "glhf", "grats", "gz", "haha", "hahaha", "hehe", "hf", "idc", "idk", "imho",
    "imo", "irl", "kek", "kekw", "lfg", "lmao", "lmfao", "lmk", "lol", "lul", "modded", "nah",
    "ngl", "np", "ofc", "ok", "omg", "omfg", "pls", "plz", "pog", "poggers", "ppl", "raided", "rn",
    "rofl", "smh", "subbed", "tbh", "tho", "thx", "ty", "tyty", "ur", "vod", "vods", "wp", "wtf",
    "wth", "xd", "ya", "yea", "yep",
];

/// The words of a message: each one's byte range, with the punctuation around it left out.
fn words(text: &str) -> Vec<(Range<usize>, &str)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices().chain([(text.len(), ' ')]) {
        match (c.is_whitespace(), start) {
            (false, None) => start = Some(i),
            (true, Some(s)) => {
                out.push((s..i, &text[s..i]));
                start = None;
            }
            _ => {}
        }
    }
    out
}

/// A word (or what's around it) the spellcheck leaves alone, whatever the dictionary says.
fn chat_only(token: &str, word: &str) -> bool {
    let first = token.chars().next().unwrap_or(' ');
    let camel =
        word.chars().skip(1).any(char::is_uppercase) && word.chars().any(char::is_lowercase);
    matches!(first, '@' | ':' | '/' | '!' | '#' | '$')
        || token.contains("://")
        || token.starts_with("www.")
        || word.chars().count() < 2
        || word.chars().any(|c| c.is_ascii_digit())
        || !word.chars().any(char::is_alphabetic)
        || camel
        || CHAT_WORDS.contains(&word.to_lowercase().as_str())
}

/// The byte ranges of the words to underline in a message. `known` tells whether a word is
/// a chat word anyway (an emote, a chatter's name, an own word). The last word, while it's
/// still being typed (no space after it), waits.
pub fn misspelled(dict: &Dicts, text: &str, known: &dyn Fn(&str) -> bool) -> Vec<Range<usize>> {
    let mut list = words(text);
    if !text.ends_with(char::is_whitespace) {
        list.pop();
    }
    list.into_iter()
        .filter_map(|(range, token)| {
            if known(token) {
                return None;
            }
            // the word without the punctuation around it ("hello," "(why)")
            let lead = token.len()
                - token
                    .trim_start_matches(|c: char| !c.is_alphanumeric())
                    .len();
            let word = token.trim_matches(|c: char| !c.is_alphanumeric());
            if chat_only(token, word) || known(word) || dict.check(word) {
                return None;
            }
            let start = range.start + lead;
            Some(start..start + word.len())
        })
        .collect()
}

/// Up to 6 suggestions for a misspelled word, from each language in turn.
pub fn suggestions(dicts: &Dicts, word: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for d in &dicts.0 {
        let mut more = Vec::new();
        d.suggest(word, &mut more);
        for s in more {
            if !out.contains(&s) {
                out.push(s);
            }
        }
    }
    out.truncate(6);
    out
}

/// Every word of a message as typed and without the punctuation around it (to look up
/// which ones are emotes or names).
pub fn word_forms(text: &str) -> Vec<&str> {
    words(text)
        .into_iter()
        .flat_map(|(_, t)| [t, t.trim_matches(|c: char| !c.is_alphanumeric())])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dict() -> Dicts {
        Dicts(vec![Arc::new(
            spellbook::Dictionary::new(EN_AFF, EN_DIC).unwrap(),
        )])
    }

    #[test]
    fn languages_are_offered_once_and_english_is_built_in() {
        let mut codes: Vec<&str> = LANGUAGES.iter().map(|(c, _)| *c).collect();
        let n = codes.len();
        codes.sort();
        codes.dedup();
        assert_eq!(codes.len(), n);
        assert_eq!(LANGUAGES[0].0, "en");
        assert!(load("xx").is_err());
    }

    /// Network: a language is downloaded and checks words (German).
    /// `cargo test -- --ignored german_is_downloaded`
    #[test]
    #[ignore]
    fn german_is_downloaded() {
        let de = Dicts(vec![Arc::new(load("de").expect("German"))]);
        let none = |_: &str| false;
        assert!(misspelled(&de, "Hallo zusammen, wie geht es euch ", &none).is_empty());
        assert_eq!(misspelled(&de, "Hallo zusamen ", &none), [6..13]);
        assert!(suggestions(&de, "zusamen").iter().any(|s| s == "zusammen"));
    }

    #[test]
    fn odd_dictionaries_are_tidied() {
        // Turkish style: numeric flags from 0
        let aff = "SET UTF-8\nFLAG num\n\nSFX 0 N 1\nSFX 0 0 de .\nSFX 1 N 1\nSFX 1 0 ler/0 .\n";
        let dic = "2\nev/0,1\nkitap/1\n";
        let (a, d) = tidy(aff, dic);
        assert!(
            a.contains("SFX 1 N 1") && a.contains("SFX 1 0 de .") && a.contains("SFX 2 0 ler/1 .")
        );
        assert_eq!(d, "2\nev/1,2\nkitap/2\n");
        // Danish style: a slash inside a word, numbers from 1 (nothing moves)
        let aff = "FLAG num\nSFX 1 Y 1\nSFX 1 0 e .\n";
        let dic = "3\nhus/1\n\"A/S\"\nbil\n";
        let (a, d) = tidy(aff, dic);
        assert_eq!(a, aff);
        assert_eq!(d, "3\nhus/1\nbil\n");
        // ordinary dictionaries are left alone
        assert_eq!(
            tidy("SET UTF-8\n", "1\nword/AB\n"),
            ("SET UTF-8\n".into(), "1\nword/AB\n".into())
        );
        // and they load
        let (a, d) = tidy(
            "SET UTF-8\nFLAG num\nSFX 0 Y 1\nSFX 0 0 de .\n",
            "1\nev/0\n",
        );
        let tr = Dicts(vec![Arc::new(spellbook::Dictionary::new(&a, &d).unwrap())]);
        assert!(tr.check("ev") && tr.check("evde") && !tr.check("evle"));
    }

    /// Network: Turkish (its flags start at 0, see `tidy`) checks words.
    /// `cargo test -- --ignored turkish_checks_words` (YAP_DICT_DIR for the folder)
    #[test]
    #[ignore]
    fn turkish_checks_words() {
        let dir = std::env::var("YAP_DICT_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir().join("yaperino-dicts"));
        let tr = Dicts(vec![Arc::new(load_into("tr", &dir).expect("Turkish"))]);
        let none = |_: &str| false;
        assert!(misspelled(&tr, "Merhaba arkadaşlar, nasılsınız bugün ", &none).is_empty());
        assert_eq!(misspelled(&tr, "Merhaba arkadaslar ", &none), [8..18]);
        assert!(suggestions(&tr, "arkadaslar")
            .iter()
            .any(|s| s == "arkadaşlar"));
    }

    /// Network: every offered language downloads and loads (into YAP_DICT_DIR, else a
    /// temporary folder). `cargo test -- --ignored every_language_loads --nocapture`
    #[test]
    #[ignore]
    fn every_language_loads() {
        let dir = std::env::var("YAP_DICT_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir().join("yaperino-dicts"));
        let mut failed = Vec::new();
        for (code, name) in LANGUAGES {
            let t = std::time::Instant::now();
            match load_into(code, &dir) {
                Ok(_) => println!("{code} {name}: ok ({:.1}s)", t.elapsed().as_secs_f32()),
                Err(e) => {
                    println!("{code} {name}: FAILED {e}");
                    failed.push(*code);
                }
            }
        }
        assert!(failed.is_empty(), "failed: {failed:?}");
    }

    #[test]
    fn misspelled_words_are_found_and_chat_is_left_alone() {
        let d = dict();
        let none = |_: &str| false;
        let text = "this is a sentnce, with a mistke ";
        let found: Vec<&str> = misspelled(&d, text, &none)
            .into_iter()
            .map(|r| &text[r])
            .collect();
        assert_eq!(found, ["sentnce", "mistke"]);
        // chat things: mentions, emotes, links, numbers, CamelCase, chat words
        let chat = "@someone :catJAM https://x.y/abc 5head PogChamp lol gg KEKW ";
        assert!(misspelled(&d, chat, &none).is_empty());
        // emotes and names the app knows (here "sadge")
        let known = |w: &str| w.eq_ignore_ascii_case("sadge");
        assert!(misspelled(&d, "Sadge ", &known).is_empty());
        // the word being typed waits until it's finished
        assert!(misspelled(&d, "hello wrld", &none).is_empty());
        assert_eq!(misspelled(&d, "hello wrld ", &none), [6..10]);
        // capitals at the start and all caps are fine for dictionary words
        assert!(misspelled(&d, "Hello WORLD ", &none).is_empty());
    }

    #[test]
    fn suggestions_for_a_misspelled_word() {
        assert!(suggestions(&dict(), "mistke")
            .iter()
            .any(|s| s == "mistake"));
    }
}
