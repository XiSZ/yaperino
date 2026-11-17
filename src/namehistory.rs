//! A Twitch user's earlier names (the usercard's "Previous names"), from logs.ivr.fi, or
//! logs.zonian.dev when that doesn't answer. Both are keyless and only get the Twitch user
//! id. Kept per user id for the session.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// A name the user had, and when it was seen (Unix seconds).
#[derive(Clone, Debug, PartialEq)]
pub struct OldName {
    pub login: String,
    pub first: Option<i64>,
    pub last: Option<i64>,
}

const SOURCES: [&str; 2] = [
    "https://logs.ivr.fi/namehistory/",
    "https://logs.zonian.dev/namehistory/",
];

/// This session's answers, per user id.
fn cache() -> &'static Mutex<HashMap<String, Vec<OldName>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Vec<OldName>>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// The names a user id had (every one the log sites know, newest first), asked once per
/// session.
pub fn load(user_id: &str) -> Result<Vec<OldName>, String> {
    if user_id.is_empty() || !user_id.chars().all(|c| c.is_ascii_digit()) {
        return Err("no Twitch user id".into());
    }
    if let Some(names) = cache().lock().ok().and_then(|c| c.get(user_id).cloned()) {
        return Ok(names);
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(10)))
        .http_status_as_error(false)
        .build()
        .into();
    let mut why = String::new();
    for source in SOURCES {
        let answer = agent
            .get(&format!("{source}{user_id}"))
            .call()
            .map_err(|e| e.to_string())
            .and_then(|mut r| {
                let status = r.status().as_u16();
                let body = r.body_mut().read_to_string().unwrap_or_default();
                if status == 200 {
                    parse(&body)
                } else {
                    Err(format!("answered {status}"))
                }
            });
        match answer {
            Ok(names) => {
                if let Ok(mut c) = cache().lock() {
                    c.insert(user_id.to_string(), names.clone());
                }
                return Ok(names);
            }
            Err(e) => why = e,
        }
    }
    Err(why)
}

/// The sites' answer: [{"user_login":"...","first_timestamp":"...","last_timestamp":"..."}],
/// without empty names or repeats, newest first.
pub fn parse(body: &str) -> Result<Vec<OldName>, String> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|_| "not a name history".to_string())?;
    let list = v.as_array().ok_or("not a name history")?;
    let mut names: Vec<OldName> = Vec::new();
    for e in list {
        let s = |k: &str| e.get(k).and_then(|x| x.as_str()).unwrap_or_default();
        let login = s("user_login").trim().to_lowercase();
        if login.is_empty() {
            continue;
        }
        let (first, last) = (
            crate::model::parse_iso8601(s("first_timestamp")),
            crate::model::parse_iso8601(s("last_timestamp")),
        );
        match names.iter_mut().find(|n| n.login == login) {
            // the same name twice: one entry from its first to its last sighting
            Some(n) => {
                n.first = n.first.into_iter().chain(first).min();
                n.last = n.last.into_iter().chain(last).max();
            }
            None => names.push(OldName { login, first, last }),
        }
    }
    names.sort_by_key(|n| std::cmp::Reverse(n.last.or(n.first)));
    Ok(names)
}

/// The names other than the current one (`current`: login or display name).
pub fn previous(names: &[OldName], current: &[&str]) -> Vec<OldName> {
    names
        .iter()
        .filter(|n| !current.iter().any(|c| c.eq_ignore_ascii_case(&n.login)))
        .cloned()
        .collect()
}

/// Sample names for screenshot mode (no network).
pub fn sample(current: &str) -> Vec<OldName> {
    let now = crate::model::now_unix();
    let day = 86_400;
    vec![
        OldName {
            login: current.to_lowercase(),
            first: Some(now - 300 * day),
            last: Some(now),
        },
        OldName {
            login: "viewer_formerly_known".into(),
            first: Some(now - 900 * day),
            last: Some(now - 301 * day),
        },
        OldName {
            login: "first_name_2019".into(),
            first: Some(now - 2000 * day),
            last: Some(now - 901 * day),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_name_history() {
        let body = r#"[
            {"user_login":"oldest","first_timestamp":"2019-01-02T10:00:00Z","last_timestamp":"2020-03-04T10:00:00Z"},
            {"user_login":"","first_timestamp":"2020-01-01T00:00:00Z","last_timestamp":"2020-01-02T00:00:00Z"},
            {"user_login":"Current_Name","first_timestamp":"2022-05-06T10:00:00.123Z","last_timestamp":"2024-07-08T10:00:00Z"},
            {"user_login":"middle","first_timestamp":"2020-03-05T10:00:00Z","last_timestamp":"2022-05-05T10:00:00Z"}
        ]"#;
        let names = parse(body).unwrap();
        let logins: Vec<&str> = names.iter().map(|n| n.login.as_str()).collect();
        assert_eq!(logins, ["current_name", "middle", "oldest"]);
        assert_eq!(
            names[2].first,
            crate::model::parse_iso8601("2019-01-02T10:00:00Z")
        );
        let earlier = previous(&names, &["Current_Name"]);
        assert_eq!(earlier.len(), 2);
        assert_eq!(earlier[0].login, "middle");
        assert_eq!(parse("[]").unwrap(), Vec::new());
        assert!(parse("<html>").is_err());
        assert!(load("not-an-id").is_err());
    }
}
