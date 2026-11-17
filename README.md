<p align="center"><img src="assets/yaperino.png" width="128" alt="Yaperino logo"></p>

# Yaperino

<p align="center">
  <a href="https://github.com/XiSZ/yaperino/actions/workflows/build.yml"><img alt="Build" src="https://img.shields.io/github/actions/workflow/status/XiSZ/yaperino/build.yml?branch=main&label=build&labelColor=4a1da8&color=7b3fe4"></a>
  <a href="https://github.com/XiSZ/yaperino/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/XiSZ/yaperino?sort=semver&label=release&labelColor=4a1da8&color=5c5be7"></a>
  <a href="https://github.com/XiSZ/yaperino/releases"><img alt="Downloads" src="https://img.shields.io/github/downloads/XiSZ/yaperino/total?label=downloads&labelColor=4a1da8&color=3e77ea"></a>
  <a href="#download-and-install"><img alt="Platforms: Windows, macOS, Linux" src="https://img.shields.io/badge/platforms-Windows%20%7C%20macOS%20%7C%20Linux-1f92ec?labelColor=4a1da8"></a>
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/github/license/XiSZ/yaperino?label=license&labelColor=4a1da8&color=00aeef"></a>
</p>

Yaperino is a desktop chat app for Windows (Linux and macOS: experimental) that puts **Twitch, Kick, YouTube and TikTok**
chat in one window. It looks and works like [Chatterino](https://github.com/Chatterino/chatterino2),
[chatterino7](https://github.com/SevenTV/chatterino7) and [Moltorino](https://github.com/EEVA-inc/Moltorino),
and adds [Chatty](https://github.com/chatty/chatty)'s **Live Streams** window: a list of the
channels you follow that are live right now, where one click opens their chat.

It is its own code, written in Rust with egui. It isn't a fork of Chatterino's C++ code.

**Website: [xisz.github.io/yaperino](https://xisz.github.io/yaperino/)**

## Features

### One chat for every platform

- Joining a channel joins it on **Twitch, Kick, YouTube and TikTok** at once, in one split.
  Its other platforms are found from the streamer's Twitch and Kick profile links, or by the
  same name. You can set them by hand in Settings > Platforms.
- **Platform logos before each message** (Twitch, Kick, YouTube, TikTok) show where it came
  from. Hover a logo for the platform's name. Turn them off in Settings > General.
  Messages can also get a platform-coloured background.
- **Choose where to send**: the logo button next to the message box opens a list of the
  platforms, each with its logo. Platforms you can't send to (not signed in there, or TikTok,
  which is read only) are greyed out; hover one to see why.

### Live Streams window (Alt+L)

Modelled on Chatty's "Live Streams" list.

- Every channel your Twitch account follows that is live, plus channels you have joined,
  followed or not. The same names are checked on Kick, YouTube and TikTok too.
- One section per platform (can be switched off), with uptime, viewers, game and title.
- Sort the list, mark favourite channels and games, and show favourites first or only
  favourites.
- Open a stream's chat as a tab, or pick several and open them as tabs or as splits of one
  new tab.
- Right-click a stream: open it in the browser or its player, open it in Streamlink, copy
  its name or link, or leave its chat.
- Live notifications: a Windows notification and sound when chosen channels go live.

### Tabs and splits like Chatterino

- Rename, pop out and close tabs (there's a "×" on the tab). Drag a tab to move it.
- "+" (Ctrl+Shift+T) opens an empty tab; add splits to it with its button or Ctrl+T.
- Right-click a tab to lock it, or lock the whole tab layout, so nothing gets moved or closed
  by accident. Tabs can have colours, be "Always visible" or compact.
- **Tab groups** (Moltorino): right-click a tab > Move to group. Each group gets a chip in the
  tab bar; click it to collapse or expand the group.
- Tabs that don't fit wrap onto more rows (on the left or right, the column scrolls).
- Split header: "name (live) - uptime - viewers - game - title". Hover it for a stream
  preview.
- Hotkeys: Ctrl+J joins a channel, Ctrl+L leaves it (also /join and /part), Ctrl+T adds a
  split, Ctrl+W closes it, Ctrl+R changes channel, Ctrl+G reopens a closed split, Ctrl+F
  searches, Ctrl+Shift+F searches all chats and Ctrl+E opens the emotes. Chatterino's other default hotkeys work too, and all
  of them can be changed in Settings > Hotkeys.

### Chat

- **Emotes** from Twitch, 7TV, BetterTTV, FrankerFaceZ and Kick, with an emote menu (Ctrl+E)
  and Tab completion for emotes and user names. The **emote carousel** shows the suggestions
  as a row of pictures that slides with each Tab press.
- **Spellcheck** (English) in the message box: misspelled words get a red line, right-click
  one for suggestions or to add it to your own words. Emotes, names and chat words are left
  alone.
- **Translation** (from Moltorino): right-click a message > Translate puts the translation
  under it; the split menu's "Auto-translate this chat" translates new messages in other
  languages. Google by default (no key), or DeepL with your own key.
- **Highlights, ignores, nicknames and custom commands**, with regex support, like Chatterino.
- **Filters** for splits (Chatterino's filter language, e.g. `message.emote_only`).
- **Chat history**: opening a Twitch chat shows its recent messages (from
  recent-messages.robotty.de, like Chatterino; can be turned off).
- **Chat logs** to files, **image upload** (paste an image into the message box), and search
  (Ctrl+F).
- **From Moltorino**: /hide and /unhide, /chatters, /export (chat to text and JSON),
  clickable #channel names and a counter for repeated messages.
- **Usercards**: click a name for their picture, roles, account date, when they followed
  the channel (for moderators), their earlier Twitch names (from logs.ivr.fi), their
  messages here and in your chat logs, and buttons to mention, hide or moderate them.
- **Replies**: right-click a Twitch message > Reply. Replies show what they answer; click
  that line to see the whole thread.
- **Search across all chats** (Ctrl+Shift+F): one search over every open chat; click a match
  to go there.
- **Copy and paste** works everywhere. Right-click a message to copy it.

### Mod tools (Twitch)

Through Twitch's API: /timeout, /ban, /warn, /clear, chat modes (/slow, /followers,
/emoteonly, /subscribers, /uniquechat, /shield), /announce, /shoutout and blocked terms.
For your own channel also /mod, /vip, /raid, /commercial, /settitle, /setgame, /marker,
polls and predictions. /modhelp lists them, and the split menu has a Moderation submenu.
In channels you moderate, the **sword** in the split header turns on mod buttons in front of
every message, and the usercard has timeout, ban and unban buttons.

Also in channels you moderate (official Twitch API, live through EventSub):

- **AutoMod**: messages AutoMod holds show up in the chat with **Allow** and **Deny**.
- **Unban requests** (/unbanrequests, or split menu > Moderation): approve or deny them,
  with an optional note; new ones are announced in the chat.
- **Mod log** (/modlogs [user]): every ban, timeout, deletion, warning and mode change,
  searchable, and in the usercard for that user. Yaperino records it while it runs (Twitch
  doesn't share older actions) and keeps the last 5,000 per channel on your computer.
- **Pins**: the pinned message shows above the chat; right-click a message > Pin message
  (until the stream ends, or 1 to 30 minutes), and Unpin on the bar.

Sign in to Twitch again after updating to grant the permissions.

### Sign-ins

- **Twitch**: device login in the browser.
- **Kick and YouTube**: the sites' own login pages, in a small sign-in window. No developer
  app is needed.
- **TikTok** is read only.

### Settings and extras

- Settings are laid out like chatterino7's, with a search box.
- Dark, Black, Light and White themes, accent colour, font and zoom.
- **Streamlink** for Twitch, Kick, YouTube and TikTok.
- **Lua plugins** in Chatterino's style (a subset of its `c2` API).
- **Settings backup** (Settings > About): back up now, restore, and a daily automatic
  backup. Backups never hold sign-ins.
- **Close to the tray** (Windows, Settings > General): closing hides Yaperino by the clock.

## Download and install

All downloads are on the [latest release](https://github.com/XiSZ/yaperino/releases/latest) page.

### Windows

1. Download **`Yaperino.exe`** and start it. Nothing else needs installing.
   Windows may warn that the program is unknown, because it isn't code-signed: click
   *More info > Run anyway*.
2. Optional: open Settings (Ctrl+P) > **About** > **Install Yaperino**. It is copied to
   `%LOCALAPPDATA%\Programs\Yaperino` and added to Start and to Settings > Apps, for your
   Windows account only (no admin rights needed). Uninstall it from Settings > Apps like any
   app.

Needs Windows 10 or 11. Kick and YouTube sign-in use Microsoft Edge WebView2, which Windows
10 and 11 already have.

### macOS (experimental)

1. Download **`Yaperino-macos.zip`** (one app for Apple Silicon and Intel Macs, macOS 11 or
   newer), unzip it and move **Yaperino** to Applications.
2. Open it. The first time, macOS blocks it because it isn't signed by an Apple developer:
   open System Settings > **Privacy & Security**, scroll down and click **Open Anyway**.

### Linux (experimental)

1. Download **`Yaperino-linux-x86_64.tar.gz`**, extract it and run `./yaperino`.
2. Signing in to Kick or YouTube opens a small sign-in window (`yaperino-login`, in the same
   folder). It needs WebKitGTK 4.1: `sudo apt install libwebkit2gtk-4.1-0` on Ubuntu /
   Debian, `sudo dnf install webkit2gtk4.1` on Fedora, `sudo pacman -S webkit2gtk-4.1` on
   Arch. Everything else, including Twitch sign-in, works without it.

Live notifications and sounds work on all three systems. The Linux and macOS versions are
new and haven't been tested on real computers yet: please report what works and what doesn't
in the [issues](https://github.com/XiSZ/yaperino/issues).

[Streamlink](https://streamlink.github.io/) is optional on every system, for "Open in
Streamlink".

**Your data stays yours:** settings and sign-ins are saved on your computer
(`%APPDATA%\Yaperino` on Windows, `~/Library/Application Support/Yaperino` on macOS,
`~/.local/share/yaperino` on Linux), and you sign in with your own Twitch, Kick and YouTube
accounts. Twitch sign-in works right away: releases include the project's Twitch app Client
ID, which is a public identifier, not a secret.

## Build

You need:

- a recent stable Rust (rustup.rs);
- a C compiler, which builds the bundled Lua:
  - Windows: the Visual Studio Build Tools with "Desktop development with C++" (its Windows
    SDK's `rc.exe` also puts the icon into the program);
  - macOS: the Xcode Command Line Tools (`xcode-select --install`);
  - Linux: `gcc` and `make` (on Ubuntu/Debian: `sudo apt install build-essential`).

```
cargo build --release
cargo test
```

The program is `target/release/live-streams.exe` (`live-streams` on Linux and macOS). The Windows
installer copies it as `Yaperino.exe`.
It keeps the older file name because Windows' ransomware protection had an exception for that name on the
development machine. Rename the `[[bin]]` in `Cargo.toml` if you don't need that.

For Twitch sign-in in your own build, add your Twitch app's Client ID (see
[Twitch app](#twitch-app-client-id)), or enter it in Settings > Accounts.

Started from the build folder, the program runs a copy of itself from
`%LOCALAPPDATA%\Yaperino\run`, so building again never fails because the program is running.
Set `YAPERINO_IN_PLACE=1` to turn that off.

**Windows ransomware protection (Controlled Folder Access).** If the project sits in a protected
folder such as Documents, Windows may block the build scripts and the C compiler from writing
there. A local `.cargo/config.toml` (kept out of git) moves the intermediate build files out of
the folder:

```toml
[build]
build-dir = "{cargo-cache-home}/build/{workspace-path-hash}"
```

The finished program still lands in `target/`.

## Development

[CLAUDE.md](CLAUDE.md) is the developer guide: source map, code patterns, the full list of
screenshot switches and the project rules. Claude Code reads it on its own.

Start options, mainly for testing:

- `--screenshot <file.png>`: draw built-in sample data (no network, made-up channels) once and
  save a picture. The `LIVE_STREAMS_SHOT_*` environment variables pick what the picture shows
  (theme, open menus, Settings pages and more; see `src/app.rs` and `src/chat_main.rs`).
  Pictures go in `shots/`, which git ignores.
- `--write-logo <folder>`: write the logo as PNG and ICO. The logo is drawn in `src/logo.rs`.
- `--web-login kick|youtube` and `--uninstall` are used by the program itself.

`LIVE_STREAMS_DEMO=1` starts the normal window with the sample data instead of live data.

## Twitch app (Client ID)

Twitch sign-in uses a registered Twitch app (Public client, no secret). Its Client ID is **not**
in the source: put it in `twitch_client_id.txt` next to `Cargo.toml` (one line), or set
`YAPERINO_TWITCH_CLIENT_ID` when building. That file is in `.gitignore`. A build without one
asks for a Client ID in Settings > Accounts. Register yours at dev.twitch.tv/console.

## Contributing

Bug reports, ideas and pull requests are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md)
for how to build, test and send changes, and the [Code of Conduct](CODE_OF_CONDUCT.md).
Report security problems privately as described in [SECURITY.md](SECURITY.md).
Releases and the checks before publishing are in [PUBLISHING.md](PUBLISHING.md).

## Credits

Yaperino is its own code (Rust, egui). Its looks and behaviour follow these open-source projects:
[Chatterino 2](https://github.com/Chatterino/chatterino2),
[chatterino7](https://github.com/SevenTV/chatterino7),
[Moltorino](https://github.com/EEVA-inc/Moltorino) and
[Chatty](https://github.com/chatty/chatty).
Emotes come from 7TV, BetterTTV and FrankerFaceZ. Live data comes from Twitch, api.ivr.fi, Kick, YouTube and TikTok.
Yaperino is not affiliated with any of them.

## License

MIT, see [LICENSE](LICENSE).
