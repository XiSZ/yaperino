//! Highlights, ignores and filters, as Chatterino's Highlights, Ignores and Filters pages.
//!
//! - A phrase without "regex" matches as a whole word (Chatterino's HighlightPhrase:
//!   word boundaries around the phrase); case doesn't matter unless "case-sensitive" is on.
//! - Ignores either drop the message (block) or replace what matched.
//! - Filters are small expressions in Chatterino's filter syntax:
//!   `author.name == "someone" || message.content contains "hello" && !(platform == "kick")`
//!   with `==`, `!=`, `contains`, `startswith`, `endswith`, `matches` (regex), `&&`, `||`,
//!   `!`, brackets, "text" and the values `author.name`, `author.badges`, `message.content`,
//!   `channel.name`, `platform`. Text comparisons ignore case.

use regex::Regex;

/// The regex for one highlight / ignore pattern (None when it isn't a valid regex).
pub fn pattern_regex(pattern: &str, is_regex: bool, case_sensitive: bool) -> Option<Regex> {
    let p = pattern.trim();
    if p.is_empty() {
        return None;
    }
    // a plain phrase matches as a whole word: a word boundary on each side that is a letter or digit
    let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    let body = if is_regex {
        p.to_string()
    } else {
        let start = if word(p.chars().next()) { r"\b" } else { "" };
        let end = if word(p.chars().last()) { r"\b" } else { "" };
        format!("{start}{}{end}", regex::escape(p))
    };
    let full = if case_sensitive {
        body
    } else {
        format!("(?i){body}")
    };
    Regex::new(&full).ok()
}

/// A compiled ignore rule.
pub struct Ignore {
    pub re: Regex,
    pub block: bool,
    pub replacement: String,
}

/// Applies ignores to a message: None = drop it, Some(text) = the (maybe changed) text.
pub fn apply_ignores(text: &str, rules: &[Ignore]) -> Option<String> {
    let mut t = text.to_string();
    for r in rules {
        if r.re.is_match(&t) {
            if r.block {
                return None;
            }
            t = r.re.replace_all(&t, r.replacement.as_str()).into_owned();
        }
    }
    Some(t)
}

/// A user name rule: exactly that name (any case), or a regex.
pub fn user_regex(pattern: &str, is_regex: bool, case_sensitive: bool) -> Option<Regex> {
    let p = pattern.trim();
    if p.is_empty() {
        return None;
    }
    let body = if is_regex {
        p.to_string()
    } else {
        format!("^{}$", regex::escape(p))
    };
    Regex::new(&if case_sensitive {
        body
    } else {
        format!("(?i){body}")
    })
    .ok()
}

/// A compiled highlight: what it colours, and whether it flashes the taskbar / plays a sound.
pub struct Rule {
    pub re: Regex,
    pub color: [u8; 3],
    pub flash: bool,
    pub sound: bool,
}

/// What a highlight does to a message.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub color: [u8; 3],
    pub flash: bool,
    pub sound: bool,
}

/// Every highlight, ignore and filter, compiled once whenever Settings change.
#[derive(Default)]
pub struct Matchers {
    pub phrases: Vec<Rule>,
    pub users: Vec<Rule>,
    /// Badge set (moderator, vip, subscriber...) -> colour, flash, sound.
    pub badges: Vec<(String, Hit)>,
    /// Users whose messages are never highlighted (lower case).
    pub blacklist: std::collections::HashSet<String>,
    pub ignores: Vec<Ignore>,
    pub ignore_users: Vec<Regex>,
    /// Filter name -> the parsed filter or why it doesn't parse.
    pub filters: std::collections::HashMap<String, Result<Expr, String>>,
}

impl Matchers {
    /// Chatterino's order: messages (phrases) first, then users, then badges.
    pub fn highlight(&self, name: &str, text: &str, badges: &[(String, String)]) -> Option<Hit> {
        if self.blacklist.contains(&name.to_lowercase()) {
            return None;
        }
        let hit = |r: &Rule| Hit {
            color: r.color,
            flash: r.flash,
            sound: r.sound,
        };
        if let Some(r) = self.phrases.iter().find(|r| r.re.is_match(text)) {
            return Some(hit(r));
        }
        if let Some(r) = self.users.iter().find(|r| r.re.is_match(name)) {
            return Some(hit(r));
        }
        self.badges
            .iter()
            .find(|(b, _)| badges.iter().any(|(set, _)| set.eq_ignore_ascii_case(b)))
            .map(|(_, h)| *h)
    }

    pub fn user_ignored(&self, name: &str) -> bool {
        self.ignore_users.iter().any(|r| r.is_match(name))
    }
}

// ------------------------------------------------------------------------- filters

/// What a filter looks at.
pub struct FilterInput<'a> {
    pub author: &'a str,
    pub content: &'a str,
    pub channel: &'a str,
    pub platform: &'a str,
    pub badges: Vec<String>,
    /// Moltorino's emote-only filter: the message is only emotes.
    pub emote_only: bool,
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Str(String),
    Ident(String),
    Op(String),
    LParen,
    RParen,
}

fn tokenize(s: &str) -> Result<Vec<Tok>, String> {
    let mut out = Vec::new();
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
        } else if ch == '"' || ch == '\'' {
            let q = ch;
            let mut v = String::new();
            i += 1;
            while i < c.len() && c[i] != q {
                if c[i] == '\\' && i + 1 < c.len() {
                    i += 1;
                }
                v.push(c[i]);
                i += 1;
            }
            if i >= c.len() {
                return Err("a text is missing its closing quote".into());
            }
            i += 1;
            out.push(Tok::Str(v));
        } else if ch == '(' {
            out.push(Tok::LParen);
            i += 1;
        } else if ch == ')' {
            out.push(Tok::RParen);
            i += 1;
        } else if "=!&|<>".contains(ch) {
            let two: String = c[i..(i + 2).min(c.len())].iter().collect();
            if ["==", "!=", "&&", "||"].contains(&two.as_str()) {
                out.push(Tok::Op(two));
                i += 2;
            } else if ch == '!' {
                out.push(Tok::Op("!".into()));
                i += 1;
            } else {
                return Err(format!("unknown operator near '{ch}'"));
            }
        } else if ch.is_alphanumeric() || ch == '_' || ch == '.' {
            let mut v = String::new();
            while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_' || c[i] == '.') {
                v.push(c[i]);
                i += 1;
            }
            let low = v.to_lowercase();
            if ["contains", "startswith", "endswith", "matches"].contains(&low.as_str()) {
                out.push(Tok::Op(low));
            } else {
                out.push(Tok::Ident(v));
            }
        } else {
            return Err(format!("unexpected '{ch}'"));
        }
    }
    Ok(out)
}

/// A parsed filter.
#[derive(Debug, Clone)]
pub enum Expr {
    Or(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Cmp(Box<Expr>, String, Box<Expr>),
    Text(String),
    Var(String),
    Bool(bool),
}

const VARS: [&str; 6] = [
    "author.name",
    "author.badges",
    "message.content",
    "channel.name",
    "platform",
    "message.emote_only",
];

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn op_is(&self, op: &str) -> bool {
        matches!(self.peek(), Some(Tok::Op(o)) if o == op)
    }
    fn or(&mut self) -> Result<Expr, String> {
        let mut e = self.and()?;
        while self.op_is("||") {
            self.pos += 1;
            e = Expr::Or(Box::new(e), Box::new(self.and()?));
        }
        Ok(e)
    }
    fn and(&mut self) -> Result<Expr, String> {
        let mut e = self.not()?;
        while self.op_is("&&") {
            self.pos += 1;
            e = Expr::And(Box::new(e), Box::new(self.not()?));
        }
        Ok(e)
    }
    fn not(&mut self) -> Result<Expr, String> {
        if self.op_is("!") {
            self.pos += 1;
            return Ok(Expr::Not(Box::new(self.not()?)));
        }
        self.cmp()
    }
    fn cmp(&mut self) -> Result<Expr, String> {
        let left = self.value()?;
        if let Some(Tok::Op(o)) = self.peek().cloned() {
            if ["==", "!=", "contains", "startswith", "endswith", "matches"].contains(&o.as_str()) {
                self.pos += 1;
                let right = self.value()?;
                return Ok(Expr::Cmp(Box::new(left), o, Box::new(right)));
            }
        }
        Ok(left)
    }
    fn value(&mut self) -> Result<Expr, String> {
        match self.peek().cloned() {
            Some(Tok::Str(s)) => {
                self.pos += 1;
                Ok(Expr::Text(s))
            }
            Some(Tok::Ident(v)) => {
                self.pos += 1;
                let low = v.to_lowercase();
                match low.as_str() {
                    "true" => Ok(Expr::Bool(true)),
                    "false" => Ok(Expr::Bool(false)),
                    _ if VARS.contains(&low.as_str()) => Ok(Expr::Var(low)),
                    _ => Err(format!("unknown value '{v}' (use {})", VARS.join(", "))),
                }
            }
            Some(Tok::LParen) => {
                self.pos += 1;
                let e = self.or()?;
                if self.peek() != Some(&Tok::RParen) {
                    return Err("a bracket isn't closed".into());
                }
                self.pos += 1;
                Ok(e)
            }
            Some(t) => Err(format!("unexpected {t:?}")),
            None => Err("the filter ends too early".into()),
        }
    }
}

/// Parses a filter, or says what's wrong with it.
pub fn parse_filter(s: &str) -> Result<Expr, String> {
    let toks = tokenize(s)?;
    if toks.is_empty() {
        return Err("empty filter".into());
    }
    let mut p = Parser { toks, pos: 0 };
    let e = p.or()?;
    if p.pos != p.toks.len() {
        return Err(format!("unexpected {:?}", p.toks[p.pos]));
    }
    Ok(e)
}

enum Val {
    Text(String),
    List(Vec<String>),
    Bool(bool),
}

fn eval_val(e: &Expr, m: &FilterInput) -> Val {
    match e {
        Expr::Text(s) => Val::Text(s.clone()),
        Expr::Bool(b) => Val::Bool(*b),
        Expr::Var(v) => match v.as_str() {
            "author.name" => Val::Text(m.author.to_string()),
            "message.content" => Val::Text(m.content.to_string()),
            "channel.name" => Val::Text(m.channel.to_string()),
            "platform" => Val::Text(m.platform.to_string()),
            "author.badges" => Val::List(m.badges.clone()),
            "message.emote_only" => Val::Bool(m.emote_only),
            _ => Val::Bool(false),
        },
        other => Val::Bool(eval(other, m)),
    }
}

/// Does a message pass the filter?
pub fn eval(e: &Expr, m: &FilterInput) -> bool {
    match e {
        Expr::Or(a, b) => eval(a, m) || eval(b, m),
        Expr::And(a, b) => eval(a, m) && eval(b, m),
        Expr::Not(a) => !eval(a, m),
        Expr::Bool(b) => *b,
        Expr::Text(s) => !s.is_empty(),
        Expr::Var(_) => match eval_val(e, m) {
            Val::Text(t) => !t.is_empty(),
            Val::List(l) => !l.is_empty(),
            Val::Bool(b) => b,
        },
        Expr::Cmp(a, op, b) => {
            let (l, r) = (eval_val(a, m), eval_val(b, m));
            let low = |s: &str| s.to_lowercase();
            match (l, r) {
                (Val::List(list), Val::Text(t)) => match op.as_str() {
                    "contains" | "==" => list.iter().any(|x| low(x) == low(&t)),
                    "!=" => !list.iter().any(|x| low(x) == low(&t)),
                    _ => false,
                },
                (Val::Text(x), Val::Text(t)) => {
                    let (x, t2) = (low(&x), low(&t));
                    match op.as_str() {
                        "==" => x == t2,
                        "!=" => x != t2,
                        "contains" => x.contains(&t2),
                        "startswith" => x.starts_with(&t2),
                        "endswith" => x.ends_with(&t2),
                        "matches" => {
                            Regex::new(&format!("(?i){t}")).is_ok_and(|re| re.is_match(&x))
                        }
                        _ => false,
                    }
                }
                (Val::Bool(x), Val::Bool(y)) => match op.as_str() {
                    "==" => x == y,
                    "!=" => x != y,
                    _ => false,
                },
                _ => false,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg<'a>(author: &'a str, content: &'a str) -> FilterInput<'a> {
        FilterInput {
            author,
            content,
            channel: "chan",
            platform: "twitch",
            badges: vec!["moderator".into()],
            emote_only: content == "KEKW",
        }
    }

    #[test]
    fn phrases_match_whole_words() {
        let re = pattern_regex("pog", false, false).unwrap();
        assert!(re.is_match("that was POG"));
        assert!(!re.is_match("pogchamp"));
        let cs = pattern_regex("Hi", false, true).unwrap();
        assert!(!cs.is_match("hi there"));
        assert!(pattern_regex("a(b", true, false).is_none());
        assert!(pattern_regex("^!", true, false).unwrap().is_match("!cmd"));
    }

    #[test]
    fn ignores_block_and_replace() {
        let rules = vec![
            Ignore {
                re: pattern_regex("darn", false, false).unwrap(),
                block: false,
                replacement: "***".into(),
            },
            Ignore {
                re: pattern_regex("spam", false, false).unwrap(),
                block: true,
                replacement: String::new(),
            },
        ];
        assert_eq!(
            apply_ignores("oh darn it", &rules).as_deref(),
            Some("oh *** it")
        );
        assert_eq!(apply_ignores("buy spam now", &rules), None);
        assert_eq!(apply_ignores("hello", &rules).as_deref(), Some("hello"));
    }

    #[test]
    fn filters_parse_and_evaluate() {
        let f =
            parse_filter(r#"author.name == "Bob" || message.content contains "hello""#).unwrap();
        assert!(eval(&f, &msg("bob", "x")));
        assert!(eval(&f, &msg("amy", "Hello there")));
        assert!(!eval(&f, &msg("amy", "bye")));
        let g =
            parse_filter(r#"!(platform == "kick") && author.badges contains "moderator""#).unwrap();
        assert!(eval(&g, &msg("amy", "x")));
        assert!(parse_filter("author.name ==").is_err());
        assert!(parse_filter("nonsense == \"x\"").is_err());
        assert!(parse_filter("(author.name == \"x\"").is_err());
    }
}
