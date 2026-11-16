# Security Policy

## Supported versions

Only the [latest release](https://github.com/XiSZ/yaperino/releases/latest) gets security
fixes. If you use an older version, update first and check whether the problem is still there.

## Reporting a vulnerability

**Please don't report security problems in public issues, pull requests or discussions.**

Report them privately through GitHub instead:
[**Report a vulnerability**](https://github.com/XiSZ/yaperino/security/advisories/new)
(repository > Security tab > "Report a vulnerability").

Please include:

- the Yaperino version (Settings > About) and your Windows version;
- what an attacker can do, and what they need for it (e.g. a crafted chat message, a
  malicious plugin, a link);
- steps to reproduce it, or a proof of concept;
- whether the problem is already public anywhere.

Never include real sign-in tokens, passwords or other people's data in a report. Use a test
account, or replace them with placeholders.

## What happens next

Yaperino is a hobby project maintained in spare time, so replies are best effort. You can
expect:

1. a reply confirming the report has been received;
2. an assessment of whether it is a vulnerability and how serious it is;
3. a fix in a new release, with the advisory published after the release, crediting you
   unless you prefer not to be named.

Please give the project a reasonable amount of time to release a fix before you share the
details publicly.

## Scope

In scope, for example:

- sign-in tokens or other secrets leaking: into logs, chat, screenshots, files outside
  `%APPDATA%\Yaperino`, or to any server other than the platform they belong to;
- a chat message, emote, badge, link or stream title that makes Yaperino run code, open
  files, or send messages or moderation actions the user didn't ask for;
- a Lua plugin breaking out of its sandbox: getting `os`, `package` or `debug` access, file
  access outside its own data folder, or network access without the "Network" permission;
- problems in the Kick / YouTube sign-in window, the installer / uninstaller, or the release
  build itself.

Out of scope:

- vulnerabilities in Twitch, Kick, YouTube, TikTok, 7TV, BetterTTV, FrankerFaceZ or other
  services Yaperino talks to: report those to the service itself;
- attacks that need someone who already controls the user's Windows account or files;
- the Windows SmartScreen warning: releases aren't code-signed (see the README);
- plugins doing what their granted permissions allow. Only install plugins you trust.
