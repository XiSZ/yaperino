# Contributing to Yaperino

Thanks for helping! Bug reports, ideas and pull requests are all welcome. By taking part you
agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md). Security problems go through
[SECURITY.md](SECURITY.md), not public issues.

## Reporting bugs and ideas

- Search the [issues](https://github.com/XiSZ/yaperino/issues) first; maybe it's already there.
- For a bug, include the Yaperino version (Settings > About), the platform and channel type
  involved (Twitch, Kick, YouTube, TikTok), what you did, what you expected and what happened.
  A screenshot helps a lot.
- For a feature from Chatterino, chatterino7, Moltorino or Chatty, link to it or describe how
  it works there. Yaperino follows their look and behaviour.
- Never post sign-in tokens, Client IDs or other private data in an issue.

## Pull requests

- Open an issue first for bigger changes, so we can agree on the approach before you spend
  time on it.
- Make a branch for your pull request instead of working on `main`.
- Keep a pull request to one topic. Small pull requests get reviewed faster.
- Describe in your own words what changed, why, and how you tested it. For anything visible,
  add a screenshot.
- Contributions are licensed under the project's [MIT License](LICENSE).

## Building and testing

See [README > Build](README.md#build) for what to install. Then:

```
cargo build
cargo test
```

`cargo test` runs the unit tests. The tests that need the network are `#[ignore]`d; they run
with `cargo test -- --ignored` and a test login in `LIVE_STREAMS_TEST_LOGIN`.

To check a change visually without signing in or touching the network, use screenshot mode:

```
target\debug\live-streams.exe --screenshot shots\test.png
```

Environment variables choose what the picture shows (`LIVE_STREAMS_SHOT_LIVE=1` for the Live
Streams window, `LIVE_STREAMS_SHOT_SETTINGS=<page>` for a Settings page and more). The full
list is in [CLAUDE.md](CLAUDE.md#screenshot-mode-how-changes-are-checked-visually).

Before opening a pull request, please check:

- `cargo fmt --check`, `cargo build` and `cargo test` pass;
- what you changed looks right in screenshot mode or the real app;
- the Live Streams window still looks like Chatty's (`LIVE_STREAMS_SHOT_LIVE=1`).

## Code guidelines

[CLAUDE.md](CLAUDE.md) is the developer guide. It has a map of the source files and the
patterns the code follows. The most important ones:

- **The UI collects, then applies.** Widgets record what was clicked in `MainActions` (chat
  window) or `RowAction` (live list), and `apply_main_actions` / `run_action` change the
  state after drawing. Add a field there instead of changing state inside a closure.
- **New setting:** add the field to `Settings` (it's `#[serde(default)]`) *and* to its
  `Default`, then a control on the right Settings page.
- **New hotkey:** add it to `hotkeys::ACTIONS`, handle it in `main_hotkeys`, and in
  `popup_hotkeys` if it makes sense in pop-out windows.
- **New message-box command:** add it to `local_command` (app commands) or
  `moderation::parse_command` (Twitch mod commands), with a unit test.
- **Formatting:** run `cargo fmt` before committing. The code uses rustfmt's default style,
  and the GitHub build fails if `cargo fmt --check` finds unformatted code.
- **Comments** explain *why*, and name the Chatterino / Moltorino / Chatty feature being
  copied.
- **No new image files for icons:** icons and the logo are drawn in code (`src/style.rs`,
  `src/logo.rs`).

## Things that won't be accepted

- Code that logs, prints or sends sign-in tokens anywhere other than the platform they
  belong to.
- Features that post automatically (spam, pyramids, self-bots).
- Committed secrets or personal data: Client IDs, tokens, names, channels, local paths.
  `twitch_client_id.txt` and the other local files are in `.gitignore` for that reason.

When testing, don't post test messages or take moderation actions in other people's
channels. Use your own channel.

## Use of AI

AI tools may help you write code, as long as:

- you understand and have reviewed every line you submit;
- you have built and tested the change yourself (see above);
- you write the pull request description and replies to reviews yourself.

You are the author of your contribution. Don't add AI tools as authors or co-authors: no
`Co-Authored-By` lines for an AI and no "Generated with ..." lines in commits or pull
requests.

## Commit messages

Start with a short summary of the change. A prefix for the area helps, e.g.
`README: ...`, `Build: ...`, `Live list: ...`. Explain the *why* in the body if it isn't
obvious.
