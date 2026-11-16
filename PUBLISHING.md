# Publishing Yaperino (maintainer notes)

The repository is on GitHub at [XiSZ/yaperino](https://github.com/XiSZ/yaperino). These notes
cover what to check before pushing and how to make a release.

## Before each push

```
git status --ignored
```

Everything that isn't listed under "Ignored files" is public once pushed. `.gitignore` keeps
these on your computer:

| Kept out | Why |
| --- | --- |
| `target/`, `*.exe`, `*.pdb` | build output |
| `shots/` | pictures from `--screenshot` |
| `twitch_client_id.txt`, `.env` | your Twitch app's Client ID and other local values |
| `*.ron`, `*.log` | local settings and logs |
| `.cargo/` | this computer's build-folder workaround (see [README > Build](README.md#build)) |
| `MECHANICS.md`, `mechanics.json`, `HANDOVER.md` | private design log and handover notes |
| `.claude/worktrees/`, `.claude/settings.local.json`, `CLAUDE.local.md` | Claude Code's working copies and personal settings |
| `.vscode/`, `.idea/`, `Thumbs.db`, `desktop.ini`, `.DS_Store` | editor and system files |

- `CLAUDE.md` (the developer guide) is meant to be public.
- Sign-ins and settings live in `%APPDATA%\Yaperino` on each user's computer, never in this
  folder.
- Commits use the name and e-mail from `git config user.name` / `user.email`. Keep GitHub's
  private "noreply" address there (GitHub > Settings > Emails) so your real e-mail never
  appears in the history.
- No AI is ever named as author or co-author (see CLAUDE.md > Rules).
- Kick and YouTube need no developer app. The optional "own app" sign-ins under Advanced
  take each user's own keys.

## Making a release

1. Make sure the last build on `main` passed (the repository's Actions tab).
2. Raise `version` in `Cargo.toml` (Settings > About shows it) and commit.
3. Tag and push the tag:

   ```
   git tag -a v0.2.0 -m "Yaperino 0.2.0"
   git push origin main v0.2.0
   ```

GitHub's workflow (`.github/workflows/build.yml`) checks formatting, runs the tests, builds
`Yaperino.exe` and attaches it to a new release on the
[Releases page](https://github.com/XiSZ/yaperino/releases). That's the file people download.

By hand instead: `cargo build --release` (with your `twitch_client_id.txt` in place), then
attach `target\release\live-streams.exe`, renamed `Yaperino.exe`, to a GitHub release.

## Twitch Client ID for GitHub's builds

Releases built by GitHub get the Twitch Client ID from the repository secret
`YAPERINO_TWITCH_CLIENT_ID` (repository **Settings > Secrets and variables > Actions**). If
you register a new Twitch app, update that secret with the new Client ID. (A Client ID isn't
a password, but this keeps it out of the code.)

## Git on this computer

Windows' ransomware protection (Controlled Folder Access) doesn't let git write inside
Documents, so the repository's own data lives in `%LOCALAPPDATA%\Yaperino-git`. The small
`.git` file here just points there. Git commands work in this folder as usual. Keep both.
