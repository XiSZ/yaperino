//! Spellcheck for the message box (Moltorino): misspelled words get a red line and a right
//! click offers suggestions. English is built in: the Hunspell dictionary from SCOWL
//! (wordlist.sourceforge.net, through github.com/wooorm/dictionaries; its licence is in
//! assets/dictionaries), checked with the spellbook crate.
//!
//! Chat isn't an essay, so a lot is left alone: emotes, chatters' names, own words added
//! with "Add to dictionary", @mentions, :emote, /commands, links, words with digits,
//! CamelCase (emote-like) words and common chat words. The word being typed is only checked
//! once it's finished.

use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

static EN_AFF: &str = include_str!("../assets/dictionaries/en_US.aff");
static EN_DIC: &str = include_str!("../assets/dictionaries/en_US.dic");

static DICT: OnceLock<Option<spellbook::Dictionary>> = OnceLock::new();
static LOADING: AtomicBool = AtomicBool::new(false);

/// The English dictionary once it's ready; the first call starts loading it in the
/// background, so the message box never waits for it.
pub fn dictionary() -> Option<&'static spellbook::Dictionary> {
    if let Some(d) = DICT.get() {
        return d.as_ref();
    }
    if !LOADING.swap(true, Ordering::SeqCst) {
        std::thread::spawn(|| {
            let _ = DICT.set(spellbook::Dictionary::new(EN_AFF, EN_DIC).ok());
        });
    }
    None
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
pub fn misspelled(
    dict: &spellbook::Dictionary,
    text: &str,
    known: &dyn Fn(&str) -> bool,
) -> Vec<Range<usize>> {
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

/// Up to 6 suggestions for a misspelled word.
pub fn suggestions(dict: &spellbook::Dictionary, word: &str) -> Vec<String> {
    let mut out = Vec::new();
    dict.suggest(word, &mut out);
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

    fn dict() -> spellbook::Dictionary {
        spellbook::Dictionary::new(EN_AFF, EN_DIC).unwrap()
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
