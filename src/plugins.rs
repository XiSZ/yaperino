//! Plugins, as Chatterino's (chatterino7 Settings > Plugins): each plugin is a folder in
//! %APPDATA%\Yaperino\Plugins with an `info.json` (name, description, authors, homepage,
//! license, version, permissions) and an `init.lua`.
//!
//! What runs here: Lua 5.4 without program or module access (no os, package, debug), and
//! a part of Chatterino's c2 API:
//! - `c2.register_command(name, function(ctx) ... end)`: a chat command; `ctx.words` are the
//!   message's words, `ctx.channel` the channel;
//! - `channel:add_system_message(text)`, `channel:send_message(text)`, `channel:get_name()`,
//!   `channel.name`;
//! - `c2.system_msg(channel_name, text)`, `c2.log(level, ...)`, `c2.LogLevel`.
//! - c2.later(function, ms);
//! - with the "Network" permission: c2.HTTPRequest.create(c2.HTTPMethod.Get, url) with
//!   :set_header, :set_payload, :set_timeout, :on_success, :on_error, :finally,
//!   :execute() (the callbacks get res:data(), res:status(), res:error());
//! - with "FilesystemRead" / "FilesystemWrite": io.open / io.lines inside the plugin's
//!   own data folder only.

use mlua::{Function, Lua, LuaOptions, RegistryKey, StdLib, Table, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

/// A plugin's info.json.
#[derive(Clone, Debug, Default)]
pub struct PluginInfo {
    /// The folder name.
    pub id: String,
    pub dir: PathBuf,
    pub name: String,
    pub description: String,
    pub authors: Vec<String>,
    pub homepage: String,
    pub license: String,
    pub version: String,
    pub permissions: Vec<String>,
    /// Why it can't be read (bad info.json, no init.lua).
    pub problem: Option<String>,
}

/// What a plugin asked for while running a command.
#[derive(Debug, Clone, PartialEq)]
pub enum Out {
    /// A grey system line in a channel's chat.
    System(String, String),
    /// A chat message to send in a channel.
    Send(String, String),
}

/// The plugins folder.
pub fn folder() -> Option<PathBuf> {
    Some(
        PathBuf::from(std::env::var_os("APPDATA")?)
            .join(crate::APP_ID)
            .join("Plugins"),
    )
}

/// Reads every plugin folder's info.json.
pub fn scan() -> Vec<PluginInfo> {
    let Some(dir) = folder() else {
        return Vec::new();
    };
    let _ = std::fs::create_dir_all(&dir);
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return out;
    };
    for e in entries.flatten() {
        let path = e.path();
        if path.is_dir() {
            out.push(read_info(&path));
        }
    }
    out.sort_by_key(|p| p.name.to_lowercase());
    out
}

fn read_info(dir: &std::path::Path) -> PluginInfo {
    let id = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut p = PluginInfo {
        id: id.clone(),
        dir: dir.to_path_buf(),
        name: id,
        ..Default::default()
    };
    match std::fs::read_to_string(dir.join("info.json"))
        .map_err(|e| e.to_string())
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).map_err(|e| e.to_string()))
    {
        Ok(v) => {
            let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
            if !s("name").is_empty() {
                p.name = s("name");
            }
            p.description = s("description");
            p.homepage = s("homepage");
            p.license = s("license");
            p.version = s("version");
            p.authors = v
                .get("authors")
                .and_then(|a| a.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            p.permissions = v
                .get("permissions")
                .and_then(|a| a.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.get("type").and_then(|t| t.as_str()).map(String::from))
                        .collect()
                })
                .unwrap_or_default();
        }
        Err(e) => p.problem = Some(format!("info.json: {e}")),
    }
    if !dir.join("init.lua").is_file() {
        p.problem = Some("no init.lua".into());
    }
    p
}

/// Words people can read for Chatterino's permission names.
pub fn permission_text(p: &str) -> &'static str {
    match p {
        "FilesystemRead" => "Read files in its data directory",
        "FilesystemWrite" => "Write files in its data directory",
        "Network" => "Make requests over the internet to third party websites",
        "ClipboardRead" => "Read the clipboard (not available in Yaperino)",
        _ => "An unknown permission (not available in Yaperino)",
    }
}

struct Loaded {
    lua: Lua,
    commands: Rc<RefCell<HashMap<String, RegistryKey>>>,
    out: Rc<RefCell<Vec<Out>>>,
    log: Rc<RefCell<Vec<String>>>,
    /// c2.later: (when, function).
    timers: Rc<RefCell<Vec<(Instant, RegistryKey)>>>,
    /// c2.HTTPRequest: request id -> (on_success, on_error, finally), and finished requests.
    pending: Rc<RefCell<HashMap<u64, [Option<RegistryKey>; 3]>>>,
    http_rx: Receiver<HttpDone>,
}

/// A finished HTTP request of a plugin.
struct HttpDone {
    id: u64,
    status: u16,
    body: String,
    error: Option<String>,
}

/// The running plugins.
#[derive(Default)]
pub struct Plugins {
    loaded: HashMap<String, Loaded>,
    /// What plugins logged (c2.log) and load errors, newest last.
    pub log: Vec<String>,
}

fn channel_table(lua: &Lua, name: &str, out: &Rc<RefCell<Vec<Out>>>) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("name", name)?;
    let (n1, o1) = (name.to_string(), out.clone());
    t.set(
        "add_system_message",
        lua.create_function(move |_, (_this, text): (Value, String)| {
            o1.borrow_mut().push(Out::System(n1.clone(), text));
            Ok(())
        })?,
    )?;
    let (n2, o2) = (name.to_string(), out.clone());
    t.set(
        "send_message",
        lua.create_function(
            move |_, (_this, text, _exec): (Value, String, Option<bool>)| {
                o2.borrow_mut().push(Out::Send(n2.clone(), text));
                Ok(())
            },
        )?,
    )?;
    let n3 = name.to_string();
    t.set(
        "get_name",
        lua.create_function(move |_, _this: Value| Ok(n3.clone()))?,
    )?;
    Ok(t)
}

/// What a plugin is allowed (from info.json "permissions").
fn has(info: &PluginInfo, perm: &str) -> bool {
    info.permissions.iter().any(|p| p == perm)
}

/// The plugin's own data folder (Chatterino: <plugin>/data); files outside it are refused.
fn resolve(data: &Path, rel: &str) -> Result<PathBuf, String> {
    let p = Path::new(rel);
    if p.is_absolute()
        || p.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir
                    | std::path::Component::Prefix(_)
                    | std::path::Component::RootDir
            )
        })
    {
        return Err(format!(
            "{rel}: plugins can only use files in their data folder"
        ));
    }
    let _ = std::fs::create_dir_all(data);
    Ok(data.join(p))
}

/// Lua part of the API: Chatterino's HTTPRequest object and the sandboxed io functions.
const PRELUDE_HTTP: &str = r#"
c2.HTTPMethod = { Get = 0, Post = 1, Put = 2, Delete = 3, Patch = 4 }
c2.HTTPRequest = {}
function c2.HTTPRequest.create(method, url)
  local r = { method = method or 0, url = url, headers = {}, payload = nil, timeout = 10000 }
  function r:set_header(k, v) self.headers[k] = v end
  function r:set_payload(p) self.payload = p end
  function r:set_timeout(ms) self.timeout = ms end
  function r:on_success(f) self.success = f end
  function r:on_error(f) self.error = f end
  function r:finally(f) self.fin = f end
  function r:execute() c2.__http(self) end
  return r
end
"#;

const PRELUDE_NO_HTTP: &str = r#"
c2.HTTPMethod = { Get = 0, Post = 1, Put = 2, Delete = 3, Patch = 4 }
c2.HTTPRequest = { create = function() error("this plugin didn't ask for the Network permission in info.json") end }
"#;

const PRELUDE_IO: &str = r#"
do
  local open, lines = io.open, io.lines
  io.open = function(p, mode)
    local abs, err = c2.__resolve(p, mode or "r")
    if not abs then return nil, err end
    return open(abs, mode)
  end
  io.lines = function(p, ...)
    local abs, err = c2.__resolve(p, "r")
    if not abs then error(err) end
    return lines(abs, ...)
  end
  io.popen = nil; io.input = nil; io.output = nil; io.tmpfile = nil; io.read = nil; io.write = nil
end
"#;

/// A result table for HTTP callbacks: res:data(), res:status(), res:error().
fn result_table(lua: &Lua, d: &HttpDone) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    let (body, status, err) = (
        d.body.clone(),
        d.status,
        d.error.clone().unwrap_or_default(),
    );
    t.set(
        "data",
        lua.create_function(move |_, _this: Value| Ok(body.clone()))?,
    )?;
    t.set(
        "status",
        lua.create_function(move |_, _this: Value| Ok(status))?,
    )?;
    t.set(
        "error",
        lua.create_function(move |_, _this: Value| Ok(err.clone()))?,
    )?;
    Ok(t)
}

impl Plugins {
    pub fn is_loaded(&self, id: &str) -> bool {
        self.loaded.contains_key(id)
    }

    /// The commands every running plugin registered.
    pub fn commands(&self) -> Vec<String> {
        let mut c: Vec<String> = self
            .loaded
            .values()
            .flat_map(|l| l.commands.borrow().keys().cloned().collect::<Vec<_>>())
            .collect();
        c.sort();
        c
    }

    pub fn commands_of(&self, id: &str) -> Vec<String> {
        let mut c: Vec<String> = self
            .loaded
            .get(id)
            .map(|l| l.commands.borrow().keys().cloned().collect())
            .unwrap_or_default();
        c.sort();
        c
    }

    pub fn unload(&mut self, id: &str) {
        self.loaded.remove(id);
    }

    /// Starts a plugin (again): runs its init.lua.
    pub fn load(&mut self, info: &PluginInfo) -> Result<(), String> {
        self.unload(&info.id);
        let code = std::fs::read_to_string(info.dir.join("init.lua")).map_err(|e| e.to_string())?;
        // no os, package or debug: a plugin can't start programs; io only with a Filesystem
        // permission, and then only inside its own data folder
        let files = has(info, "FilesystemRead") || has(info, "FilesystemWrite");
        let mut libs =
            StdLib::TABLE | StdLib::STRING | StdLib::MATH | StdLib::UTF8 | StdLib::COROUTINE;
        if files {
            libs |= StdLib::IO;
        }
        let lua = Lua::new_with(libs, LuaOptions::default()).map_err(|e| e.to_string())?;
        let commands: Rc<RefCell<HashMap<String, RegistryKey>>> = Rc::default();
        let out: Rc<RefCell<Vec<Out>>> = Rc::default();
        let log: Rc<RefCell<Vec<String>>> = Rc::default();
        let timers: Rc<RefCell<Vec<(Instant, RegistryKey)>>> = Rc::default();
        let pending: Rc<RefCell<HashMap<u64, [Option<RegistryKey>; 3]>>> = Rc::default();
        let (http_tx, http_rx) = std::sync::mpsc::channel::<HttpDone>();
        let setup = || -> mlua::Result<()> {
            let c2 = lua.create_table()?;
            let cmds = commands.clone();
            c2.set(
                "register_command",
                lua.create_function(move |lua, (name, f): (String, Function)| {
                    let key = lua.create_registry_value(f)?;
                    let name = name.trim().to_string();
                    let exists = cmds.borrow().contains_key(&name);
                    if !exists {
                        cmds.borrow_mut().insert(name, key);
                    }
                    Ok(!exists)
                })?,
            )?;
            let o = out.clone();
            c2.set(
                "system_msg",
                lua.create_function(move |_, (channel, text): (String, String)| {
                    o.borrow_mut().push(Out::System(channel, text));
                    Ok(true)
                })?,
            )?;
            let (l, id) = (log.clone(), info.id.clone());
            c2.set(
                "log",
                lua.create_function(move |_, (level, rest): (Value, mlua::Variadic<Value>)| {
                    let lvl = match level {
                        Value::Integer(0) => "debug",
                        Value::Integer(2) => "warning",
                        Value::Integer(3) => "critical",
                        _ => "info",
                    };
                    let text: Vec<String> = rest
                        .iter()
                        .map(|v| v.to_string().unwrap_or_default())
                        .collect();
                    l.borrow_mut()
                        .push(format!("[{id}] {lvl}: {}", text.join(" ")));
                    Ok(())
                })?,
            )?;
            let levels = lua.create_table()?;
            for (k, v) in [("Debug", 0), ("Info", 1), ("Warning", 2), ("Critical", 3)] {
                levels.set(k, v)?;
            }
            c2.set("LogLevel", levels)?;
            // c2.later(function, milliseconds): runs the function once, later
            let t = timers.clone();
            c2.set(
                "later",
                lua.create_function(move |lua, (f, ms): (Function, u64)| {
                    t.borrow_mut().push((
                        Instant::now() + Duration::from_millis(ms),
                        lua.create_registry_value(f)?,
                    ));
                    Ok(())
                })?,
            )?;
            // HTTP (Network permission): the request runs in the background; its callbacks
            // run on a later frame
            let (p, tx) = (pending.clone(), http_tx.clone());
            let next_id = Rc::new(RefCell::new(0u64));
            c2.set(
                "__http",
                lua.create_function(move |lua, req: Table| {
                    let method = match req.get::<i64>("method").unwrap_or(0) {
                        1 => "POST",
                        2 => "PUT",
                        3 => "DELETE",
                        4 => "PATCH",
                        _ => "GET",
                    };
                    let url: String = req.get("url")?;
                    let payload: Option<String> = req.get("payload").ok().flatten();
                    let timeout: u64 = req.get("timeout").unwrap_or(10_000);
                    let mut headers: Vec<(String, String)> = Vec::new();
                    if let Ok(h) = req.get::<Table>("headers") {
                        for (k, v) in h.pairs::<String, String>().flatten() {
                            headers.push((k, v));
                        }
                    }
                    let key = |name: &str| -> mlua::Result<Option<RegistryKey>> {
                        match req.get::<Option<Function>>(name)? {
                            Some(f) => Ok(Some(lua.create_registry_value(f)?)),
                            None => Ok(None),
                        }
                    };
                    let id = {
                        let mut n = next_id.borrow_mut();
                        *n += 1;
                        *n
                    };
                    p.borrow_mut()
                        .insert(id, [key("success")?, key("error")?, key("fin")?]);
                    let tx = tx.clone();
                    std::thread::spawn(move || {
                        let agent: ureq::Agent = ureq::Agent::config_builder()
                            .timeout_global(Some(Duration::from_millis(
                                timeout.clamp(500, 120_000),
                            )))
                            .http_status_as_error(false)
                            .user_agent("Yaperino-plugin")
                            .build()
                            .into();
                        let mut b = ureq::http::Request::builder().method(method).uri(&url);
                        for (k, v) in &headers {
                            b = b.header(k.as_str(), v.as_str());
                        }
                        let done = match b
                            .body(payload.unwrap_or_default())
                            .map_err(|e| e.to_string())
                            .and_then(|r| agent.run(r).map_err(|e| e.to_string()))
                        {
                            Ok(mut r) => {
                                let status = r.status().as_u16();
                                let body = r.body_mut().read_to_string().unwrap_or_default();
                                HttpDone {
                                    id,
                                    status,
                                    body,
                                    error: None,
                                }
                            }
                            Err(e) => HttpDone {
                                id,
                                status: 0,
                                body: String::new(),
                                error: Some(e),
                            },
                        };
                        let _ = tx.send(done);
                    });
                    Ok(())
                })?,
            )?;
            // files: only inside the plugin's data folder; writing only with FilesystemWrite
            let (data, can_write) = (info.dir.join("data"), has(info, "FilesystemWrite"));
            c2.set(
                "__resolve",
                lua.create_function(move |_, (path, mode): (String, String)| {
                    if !can_write && mode.contains(['w', 'a', '+']) {
                        return Ok((
                            None,
                            Some("this plugin may only read files (FilesystemRead)".to_string()),
                        ));
                    }
                    match resolve(&data, &path) {
                        Ok(p) => Ok((Some(p.to_string_lossy().into_owned()), None)),
                        Err(e) => Ok((None, Some(e))),
                    }
                })?,
            )?;
            lua.globals().set("c2", c2)?;
            lua.load(if has(info, "Network") {
                PRELUDE_HTTP
            } else {
                PRELUDE_NO_HTTP
            })
            .exec()?;
            if files {
                lua.load(PRELUDE_IO).exec()?;
            }
            lua.load(&code)
                .set_name(format!("{}/init.lua", info.id))
                .exec()
        };
        let r = setup();
        self.log.extend(log.borrow_mut().drain(..));
        match r {
            Ok(()) => {
                self.loaded.insert(
                    info.id.clone(),
                    Loaded {
                        lua,
                        commands,
                        out,
                        log,
                        timers,
                        pending,
                        http_rx,
                    },
                );
                Ok(())
            }
            Err(e) => {
                let msg = format!("[{}] {e}", info.id);
                self.log.push(msg.clone());
                Err(msg)
            }
        }
    }

    /// Each frame: runs due c2.later timers and the callbacks of finished HTTP requests;
    /// returns what the plugins asked for.
    pub fn poll(&mut self) -> Vec<Out> {
        let mut outs = Vec::new();
        let now = Instant::now();
        for (id, l) in &self.loaded {
            let due: Vec<RegistryKey> = {
                let mut t = l.timers.borrow_mut();
                let (d, keep): (Vec<_>, Vec<_>) = t.drain(..).partition(|(at, _)| *at <= now);
                *t = keep;
                d.into_iter().map(|(_, k)| k).collect()
            };
            for k in due {
                if let Ok(f) = l.lua.registry_value::<Function>(&k) {
                    if let Err(e) = f.call::<()>(()) {
                        self.log.push(format!("[{id}] c2.later: {e}"));
                    }
                }
                let _ = l.lua.remove_registry_value(k);
            }
            while let Ok(d) = l.http_rx.try_recv() {
                let Some(keys) = l.pending.borrow_mut().remove(&d.id) else {
                    continue;
                };
                let [success, error, fin] = keys;
                let ok = d.error.is_none() && (200..300).contains(&d.status);
                let which = if ok { success } else { error };
                if let Some(k) = which {
                    let r = l
                        .lua
                        .registry_value::<Function>(&k)
                        .and_then(|f| f.call::<()>(result_table(&l.lua, &d)?));
                    if let Err(e) = r {
                        self.log.push(format!("[{id}] HTTP callback: {e}"));
                    }
                }
                if let Some(k) = fin {
                    if let Err(e) = l
                        .lua
                        .registry_value::<Function>(&k)
                        .and_then(|f| f.call::<()>(()))
                    {
                        self.log.push(format!("[{id}] HTTP finally: {e}"));
                    }
                }
            }
            self.log.extend(l.log.borrow_mut().drain(..));
            outs.extend(l.out.borrow_mut().drain(..));
        }
        outs
    }

    /// Runs a plugin command if `text` starts with one; returns what the plugin asked for.
    pub fn run_command(&mut self, text: &str, channel: &str) -> Option<Vec<Out>> {
        let words: Vec<String> = text.split_whitespace().map(String::from).collect();
        let first = words.first()?.clone();
        for (id, l) in &self.loaded {
            let f: Option<Function> = l
                .commands
                .borrow()
                .get(&first)
                .and_then(|k| l.lua.registry_value(k).ok());
            let Some(f) = f else { continue };
            let call = || -> mlua::Result<()> {
                let ctx = l.lua.create_table()?;
                ctx.set("words", l.lua.create_sequence_from(words.clone())?)?;
                ctx.set("channel", channel_table(&l.lua, channel, &l.out)?)?;
                f.call::<()>(ctx)
            };
            if let Err(e) = call() {
                self.log.push(format!("[{id}] {first}: {e}"));
                l.out.borrow_mut().push(Out::System(
                    channel.to_string(),
                    format!("Plugin command {first} failed: {e}"),
                ));
            }
            self.log.extend(l.log.borrow_mut().drain(..));
            return Some(l.out.borrow_mut().drain(..).collect());
        }
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_a_command_plugin() {
        let dir = std::env::temp_dir().join(format!("yaperino-plugin-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("info.json"), r#"{"name":"Hello","version":"1.0","authors":["me"],"license":"MIT","permissions":[{"type":"Network"}]}"#).unwrap();
        std::fs::write(
            dir.join("init.lua"),
            r#"
            c2.log(c2.LogLevel.Info, "loaded")
            c2.register_command("/hello", function(ctx)
                ctx.channel:add_system_message("hi " .. (ctx.words[2] or "you"))
                ctx.channel:send_message("hello from " .. ctx.channel:get_name())
            end)
            assert(io == nil and os == nil, "no io/os")
            "#,
        )
        .unwrap();
        let info = read_info(&dir);
        assert_eq!(info.name, "Hello");
        assert_eq!(info.permissions, vec!["Network"]);
        let mut p = Plugins::default();
        p.load(&info).unwrap();
        assert_eq!(p.commands(), vec!["/hello"]);
        let out = p.run_command("/hello bob", "chan").unwrap();
        assert_eq!(
            out,
            vec![
                Out::System("chan".into(), "hi bob".into()),
                Out::Send("chan".into(), "hello from chan".into())
            ]
        );
        assert!(p.run_command("/other", "chan").is_none());
        assert!(p.log.iter().any(|l| l.contains("loaded")));
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn timers_files_and_permissions() {
        let dir =
            std::env::temp_dir().join(format!("yaperino-plugin-test2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("info.json"), r#"{"name":"Files","permissions":[{"type":"FilesystemRead"},{"type":"FilesystemWrite"}]}"#).unwrap();
        std::fs::write(
            dir.join("init.lua"),
            r#"
            local f = assert(io.open("note.txt", "w")); f:write("saved"); f:close()
            local g = assert(io.open("note.txt", "r")); local t = g:read("a"); g:close()
            assert(t == "saved")
            assert(io.open("../escape.txt", "w") == nil, "no leaving the data folder")
            assert(io.popen == nil)
            assert(not pcall(function() c2.HTTPRequest.create(c2.HTTPMethod.Get, "https://example.com") end), "no Network permission")
            c2.later(function() c2.system_msg("chan", "later ran") end, 0)
            "#,
        )
        .unwrap();
        let info = read_info(&dir);
        let mut p = Plugins::default();
        p.load(&info).unwrap();
        assert!(dir.join("data").join("note.txt").is_file());
        assert!(!dir.join("escape.txt").exists());
        let out = p.poll();
        assert_eq!(out, vec![Out::System("chan".into(), "later ran".into())]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
