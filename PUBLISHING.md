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

## The website

https://xisz.github.io/yaperino/ is built from `site/` and published by
`.github/workflows/pages.yml` (GitHub Pages, free). It updates by itself:

- after every release: the version, the download buttons (they always point to the latest
  release) and the changelog (the commit titles of each release; commits starting with
  `Version`, `README:`, `CLAUDE.md:` or `Build:` are left out);
- when anything in `site/` changes on `main`.

To change the text, edit `site/index.html`. The screenshots in `site/img/` come from the
app's screenshot mode (see CLAUDE.md). Preview locally: `python site/build.py`, then open
`_site/index.html` through a local web server (`python -m http.server --directory _site`).

## Twitch Client ID for GitHub's builds

Releases built by GitHub get the Twitch Client ID from the repository secret
`YAPERINO_TWITCH_CLIENT_ID` (repository **Settings > Secrets and variables > Actions**). If
you register a new Twitch app, update that secret with the new Client ID. (A Client ID isn't
a password, but this keeps it out of the code.)

## Git on this computer

The project is a normal clone in `C:\dev\yaperino`. Keep it outside Documents: Windows'
ransomware protection (Controlled Folder Access) blocks git and the build there. The private
files that are never pushed (`twitch_client_id.txt`, `HANDOVER.md`, `MECHANICS.md`) live only
in this folder; copy them along if the folder ever moves.

## Visitors, clones and downloads

- GitHub's own numbers: the repository's **Insights > Traffic** page (last 14 days only).
- The download badge at the top of the README counts all release downloads.
- `.github/workflows/traffic.yml` saves views, clones, referrers, popular pages and download
  counts once a day as CSV files on the `traffic-data` branch, so they're kept for good.
  Traffic needs the repository secret `TRAFFIC_TOKEN`: a fine-grained personal access token
  for this repository only, with "Administration: Read-only". When it expires, create a new
  one and replace the secret; until then only download counts are saved (the run shows a
  warning). Run it by hand any time: Actions > traffic > Run workflow.
