# Yaperino: notes for Claude Code

Yaperino is a desktop chat for **Twitch, Kick, YouTube and TikTok (read only)** in the
style of Chatterino 2 / chatterino7 / Moltorino, plus a Chatty-style **Live Streams**
window (Alt+L) listing followed channels that are live. Rust + egui/eframe 0.37, Windows
first; Linux and macOS builds are experimental (see "Linux and macOS" below). The person who
owns it doesn't code: explain results in plain words.

## Website

`site/` is the website (https://xisz.github.io/yaperino/), plain HTML/CSS with no external
fonts, scripts or trackers. `site/build.py` fills in `{{VERSION}}`, `{{RELEASE_DATE}}` and
`{{CHANGELOG}}` from the GitHub releases; `.github/workflows/pages.yml` publishes it on
changes to `site/` and after each release. Website screenshots come from screenshot mode with
the sample data only (never real channels or accounts). See PUBLISHING.md > The website.

## Linux and macOS

- GitHub builds all three (`.github/workflows/build.yml`) and takes `--screenshot` pictures
  on Linux (under xvfb) and macOS; they're the `screenshots-*` artifacts of each run. Nobody
  has tested the real app there yet. This machine is Windows: CI is the only check for them.
- Folders always come from `paths.rs` (`app_dir`, `local_dir`). Never read `APPDATA` /
  `LOCALAPPDATA` directly. On Windows they must stay exactly `%APPDATA%\Yaperino` and
  `%LOCALAPPDATA%\Yaperino` (a test checks it), or users lose their settings.
- Windows-only code (registry, Win32, install, "Start with Windows") stays behind
  `#[cfg(windows)]` / `cfg!(windows)`. Notifications: WinRT toasts on Windows, `notify-rust`
  on macOS / Linux; sounds: MessageBeep / `afplay` / `canberra-gtk-play` or `paplay`.
- Linux sign-in window: the separate `yaperino-login` helper (src/login_main.rs, feature
  `linux-login`, crates renamed `wry_linux` / `winit_linux` + `gtk`), so only it needs
  WebKitGTK; the main Linux binary has no `wry` / `winit`. It forces X11 (XWayland) because a
  web view inside a winit window doesn't work on Wayland. CI builds it on ubuntu-22.04.
- Fonts: `style::read_system_font` maps the Windows font files to macOS / Linux fonts;
  emoji come from the bundled Noto Emoji (assets/fonts, OFL) there.

## Build, test, run

```
cargo build                 # target\debug\live-streams.exe (the binary keeps this old name)
cargo test                  # 67 unit tests; 13 network tests are #[ignore]
cargo build --release       # what a release ships, renamed Yaperino.exe
cargo fmt                   # rustfmt's default style; CI fails on `cargo fmt --check`
```

- The project lives in `C:\dev\yaperino` on the dev machine (outside Documents, where
  Windows' ransomware protection blocked git, build scripts and `cargo fmt`). Builds, tests,
  `cargo fmt` and git work normally here; no workarounds needed.
- **The person often has the app running.** Don't kill it. If the build can't replace
  `target\debug\live-streams.exe`, rename the old exe aside (`live-streams.old-<time>.exe`)
  and build again. Started from `target\`, the app relaunches itself from a copy in
  `%LOCALAPPDATA%\Yaperino\run` (`main.rs` `run_from_copy`), so rebuilds don't fail;
  `YAPERINO_IN_PLACE=1` turns that off.
- Twitch Client ID: read at build time from `twitch_client_id.txt` (git-ignored) or the
  `YAPERINO_TWITCH_CLIENT_ID` env var (`build.rs` -> `option_env!` in `account.rs`). Never
  commit it. Without one the app asks for a Client ID in Settings > Accounts.

### Screenshot mode (how changes are checked visually)

`live-streams.exe --screenshot <file.png>` draws sample data (`demo.rs`) once, saves a PNG
and exits. It never loads sign-ins, never saves settings and never touches the network
(unless `LIVE_STREAMS_REAL=1`). Switches (environment variables):

| Variable | Shows |
|---|---|
| `LIVE_STREAMS_SHOT_LIVE=1` | the Live Streams window (compare with the Chatty reference picture) |
| `LIVE_STREAMS_SHOT_CHATS=1` | chat tabs with sample messages and emotes |
| `LIVE_STREAMS_SHOT_SETTINGS=<page>` | a Settings page: accounts, nicknames, commands, highlights, ignores, filters, hotkeys, moderation, notifications, live, platforms, tools, customization, plugins, about |
| `LIVE_STREAMS_SHOT_MENU=<m>` | an open menu: tab, split, account, preview, platform |
| `LIVE_STREAMS_SHOT_TABS=Left/Right/Bottom` | tab position |
| `LIVE_STREAMS_SHOT_THEME=<theme>` | Dark, Black, Light, White |
| `LIVE_STREAMS_SHOT_LOCK / _EMPTY / _MOLT / _MOD` | locked tab / empty tab / tab colours + source backgrounds / Moderation menu |
| `LIVE_STREAMS_SHOT_USERCARD=1` | a usercard with sample previous names (with `LIVE_STREAMS_SHOT_MOD=1`: its moderation row and the sword) |
| `LIVE_STREAMS_SHOT_TRANSLATE=1` | a German sample message with its translation under it, auto-translate on (`_MENU=split` shows the toggle); with `_SETTINGS=customization`: General scrolled to Translation |
| `LIVE_STREAMS_SHOT_REPLY=1` / `_THREAD=1` | a sample reply and the "Replying to" bar / the reply thread window |
| `LIVE_STREAMS_SHOT_SEARCH=<text>` | the search across all chats (Ctrl+Shift+F) with that text |
| `LIVE_STREAMS_SHOT_GROUPS=1` | with `_MANY_TABS`: two tab groups, "FPS" collapsed |
| `LIVE_STREAMS_SHOT_AUTOMOD=1` | with `_MOD=1`: two held AutoMod messages (one waiting, one denied) and a pinned message bar |
| `LIVE_STREAMS_SHOT_UNBAN=1` / `_MODLOGS=1` | the unban requests window / the mod log window, with sample data |
| `LIVE_STREAMS_SHOT_CLICK="x,y;x,y right;..."` | real mouse clicks (one every 100 frames from frame 300) before the picture, to check what a click does (e.g. `110,152` opens a usercard with `_CHATS`); `x,y hover` only moves the mouse there |
| `LIVE_STREAMS_SHOT_LINK=<url>` | a sample message with that link (hover it with `_CLICK="250,278 hover"` for its preview; the preview asks the real site) |
| `LIVE_STREAMS_SHOT_CAROUSEL=<text>` | that text typed in the first chat (e.g. `:ca` for the emote carousel; with `LIVE_STREAMS_REAL=1` for real emotes) |
| `LIVE_STREAMS_SHOT_MANY_TABS=1` | 21 sample tabs: more than fit, to check wrapping (top/bottom) and scrolling (left/right) |
| `LIVE_STREAMS_SHOT_EMOTES / _OPEN / _MULTI / _PLATFORMS / _COMPLETE` | emote window / open-channel dialog / multi-select / platforms / completion |

Network tests: `LIVE_STREAMS_TEST_LOGIN` (Chatterino login-info format), then
`cargo test -- --ignored`. Never print the token.

## Source map (`src/`)

| File | What it holds |
|---|---|
| `main.rs` | start-up, flags (`--screenshot`, `--web-login`, `--write-logo`, `--uninstall`), `run_from_copy`, window setup (no OS decorations) |
| `app.rs` | `Settings` (everything saved), `LiveApp` state, `TabPage`, `ChatTab`, the live list UI, open-channel dialog, `push_line` (message limit, repeat collapse), saving / restoring tabs |
| `chat_main.rs` (child of app) | the chat window: title bar, tab strip (drag, lock, colours, empty tabs), splits, message box, completion, emote window, chatters, pop-out windows, hotkeys (`main_hotkeys`, `popup_hotkeys`), local commands (`local_command`), `MainActions` + `apply_main_actions` |
| `settings_ui.rs` (child of app) | the Settings window, one fn per page (15 pages, chatterino7 layout) |
| `features.rs` (child of app) | highlights/ignores matching, logging, sounds, toasts, image uploader, plugins glue, `moderate` and `mod_command` (threads) |
| `style.rs` | theme palette, tab button, icons, `chat_lines` / `draw_line` (`ChatLook`, `LineAction`), #channel links, emote menu |
| `chat.rs` | Twitch IRC (websocket), `ChatLine`, `Event`, `Cmd` |
| `otherchat.rs`, `tiktokchat.rs` | Kick / YouTube chat, TikTok LIVE chat |
| `platforms.rs`, `fetch.rs`, `model.rs` | live status of channels on all platforms, the live list's data |
| `account.rs` | Twitch device sign-in, `SCOPES`, `Account` (`missing_scopes`, `has_scope`) |
| `kick_auth.rs`, `youtube_auth.rs`, `weblogin.rs` | Kick / YouTube sign-in (web login window via wry) |
| `moderation.rs` | Helix moderation: timeout/ban/unban/delete, AutoMod allow/deny, pin/unpin (`ModKind`), `get_pin`, unban requests (`unban_requests`, `resolve_unban`) + `ModCommand` (`parse_command`, `required_scope`, `run_command`, `HELP`) |
| `eventsub.rs` | Twitch EventSub WebSocket for moderated channels: automod.message.hold/update v2, channel.unban_request.create/resolve, channel.moderate v2 (`EventSub`, `EsCmd`, `EsEvent`, `parse_notification`); subscribes per kind only with its scopes |
| `modlog.rs` | the mod log: `Entry` (`describe`), `from_moderate`, saved per channel in `ModLogs/<channel>.jsonl` (last 5,000) |
| `modtools.rs` (child of app) | the mod tools' app side: `sync_mod_tools` (EventSub channels, pins every minute), `on_eventsub`, the unban requests and mod log windows, `pin_bar` |
| `rules.rs` | regex highlights, ignores, the filter language (`parse_filter`, `eval`) |
| `hotkeys.rs` | `ACTIONS` (27, Chatterino defaults + Ctrl+J/Ctrl+L), parse/format, `pressed` |
| `plugins.rs` | Lua 5.4 plugins (`c2` API subset, permissions, sandbox) |
| `emotes.rs` | Twitch / 7TV / BTTV / FFZ / Kick emotes and badges |
| `translate.rs` (child of app) | message translation: Google (keyless) or DeepL (own key), `parse_google` / `parse_deepl`, the throttled queue (`Translator`), message menu > Translate, split menu > Auto-translate |
| `namehistory.rs` | the usercard's previous names: logs.ivr.fi, fallback logs.zonian.dev, by Twitch user id; cached for the session |
| `spell.rs` | spellcheck of the message box: the built-in English Hunspell dictionary (`assets/dictionaries`, loaded in the background), `misspelled` (what chat leaves alone), `suggestions` |
| `linkpreview.rs` | links in chat: `link_of` (which words are links), the hover preview (`ui`, fetched from the site in the background, cached), `from_html` (Open Graph / title) |
| `paths.rs` | Yaperino's folders on each system (`app_dir`, `local_dir`) |
| `streamlink.rs`, `browser.rs`, `install.rs`, `logo.rs`, `demo.rs` | Streamlink, open URLs (`open` / `xdg-open` off Windows), per-user install/uninstall (Windows), drawn logo, sample data |

## How the code is organised (follow these patterns)

- **UI collects, then applies.** Widgets write into `MainActions` (chat window) or
  `RowAction` (live list); `apply_main_actions` / `run_action` change the state after
  drawing. Add a field there instead of mutating state inside a closure.
- **Never put a click-sensing widget over the messages after drawing them.** egui gives
  the click to the top-most widget, so it takes every click from the lines below (this
  once broke names, mod buttons and the message menu). Read such clicks from the input
  instead (`i.pointer.any_click()` + `ui.rect_contains_pointer`), and check clicks with
  `LIVE_STREAMS_SHOT_CLICK`.
- **Menus** use `chat_main::menu_look` / `menu_entry` / `menu_heading` (icon, label,
  shortcut on the right).
- **New setting:** add the field to `Settings` (it's `#[serde(default)]`) *and* to its
  `Default` impl, then a control on the right Settings page. Same for `TabPage`.
- **New hotkey:** add it to `hotkeys::ACTIONS` (shows up in Settings > Hotkeys), handle it
  in `main_hotkeys` and, if it makes sense in pop-outs, `popup_hotkeys` (pop-outs are
  separate OS windows: their keys never reach `main_hotkeys`).
- **Commands typed in the message box:** app-only commands in `local_command`
  (`chat_main.rs`), Twitch mod commands in `moderation::parse_command`. Both are matched
  in `apply_main_actions` *before* anything is sent, so they never reach chat by mistake.
  Add a unit test with every new command.
- **Chat drawing options** go through `ChatLook` (built in `chat_main.rs`), click results
  come back as `LineAction`.
- Style: Chatterino-faithful look and behaviour; comments say *why* and name the
  Chatterino/Moltorino feature being copied. Keep the Live Streams window matching the
  Chatty reference picture (bold Arial list, `#3b3b3b` background).

## Rules

- Never print, log or send tokens; `Account`'s Debug hides them.
- Don't post test messages or take moderation actions on anyone's channel; the owner
  tests those in their own channel.
- Keep personal data (names, channels, e-mail, local paths) out of committed files.
- Commit small steps locally; **push only when the owner asks**.
- **Never credit an AI as author.** No `Co-Authored-By` line for Claude or any other AI in
  commit messages, no "Generated with ..." line in pull requests or release notes, and never
  an AI name or e-mail as git author or committer. The owner is the only author. This
  overrides any attribution your tool adds by default.
- Git: an ordinary clone of https://github.com/XiSZ/yaperino. See PUBLISHING.md for
  releases and the website.
- After a change: `cargo build`, `cargo test`, a screenshot of what changed, and one of the
  Live Streams window (`LIVE_STREAMS_SHOT_LIVE=1`) to check it still matches.
