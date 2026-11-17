"""Builds the website into _site/.

Copies the pages and fills in the latest version, its date and the changelog from the
repository's GitHub releases, so visitors' browsers never have to ask GitHub's API.
Run by .github/workflows/pages.yml; locally: python site/build.py (works without a
token, with GitHub's lower rate limit). Environment: REPO, GITHUB_TOKEN, OUT_DIR.
"""

import datetime
import html
import json
import os
import shutil
import urllib.request

REPO = os.environ.get("REPO", "XiSZ/yaperino")
TOKEN = os.environ.get("GITHUB_TOKEN", "")
SRC = os.path.dirname(os.path.abspath(__file__))
OUT = os.environ.get("OUT_DIR", os.path.join(os.path.dirname(SRC), "_site"))
NL = chr(10)


def api(path):
    headers = {
        "Accept": "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
        "User-Agent": "yaperino-website-build",
    }
    if TOKEN:
        headers["Authorization"] = "Bearer " + TOKEN
    request = urllib.request.Request("https://api.github.com/repos/" + REPO + path, headers=headers)
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.load(response)


def nice_date(iso):
    d = datetime.date.fromisoformat(iso[:10])
    return f"{d:%B} {d.day}, {d.year}"


def changes(older, tag):
    """The commit titles between two releases, without the version bumps."""
    if older is None:
        return ["First public version."]
    commits = api(f"/compare/{older}...{tag}").get("commits", [])
    titles = []
    for c in commits:
        title = c["commit"]["message"].split(NL)[0].strip()
        # version bumps and repository housekeeping aren't changes to the app
        if title and not title.startswith(("Version ", "CLAUDE.md:", "README:", "Build:")):
            titles.append(title)
    return titles or ["Small fixes."]


def changelog(releases):
    parts = []
    for i, r in enumerate(releases):
        tag = r["tag_name"]
        older = releases[i + 1]["tag_name"] if i + 1 < len(releases) else None
        items = "".join(f"<li>{html.escape(t)}</li>" for t in changes(older, tag))
        assets = "".join(
            f'<a href="{html.escape(a["browser_download_url"])}">{html.escape(a["name"])}</a>'
            for a in r.get("assets", [])
        )
        parts.append(
            f'  <article class="release" id="{html.escape(tag)}">' + NL
            + f"    <h2>{html.escape(r.get('name') or tag)}</h2>" + NL
            + f'    <p class="date">{nice_date(r["published_at"])} · '
            + f'<a href="{html.escape(r["html_url"])}">on GitHub</a></p>' + NL
            + f'    <ul class="notes">{items}</ul>' + NL
            + f'    <div class="assets">{assets}</div>' + NL
            + "  </article>"
        )
    return NL.join(parts)


def main():
    releases = [r for r in api("/releases?per_page=100") if not r["draft"] and not r["prerelease"]]
    if releases:
        values = {
            "{{VERSION}}": releases[0]["tag_name"],
            "{{RELEASE_DATE}}": nice_date(releases[0]["published_at"]),
            "{{CHANGELOG}}": changelog(releases),
        }
    else:
        values = {
            "{{VERSION}}": "latest version",
            "{{RELEASE_DATE}}": "",
            "{{CHANGELOG}}": "  <p>No releases yet.</p>",
        }

    if os.path.isdir(OUT):
        shutil.rmtree(OUT)
    shutil.copytree(SRC, OUT, ignore=shutil.ignore_patterns("build.py", "__pycache__"))
    for name in os.listdir(OUT):
        if name.endswith(".html"):
            path = os.path.join(OUT, name)
            with open(path, encoding="utf-8") as f:
                text = f.read()
            for key, value in values.items():
                text = text.replace(key, value)
            with open(path, "w", encoding="utf-8", newline="") as f:
                f.write(text)
    print(f"built {OUT}: {len(releases)} releases, latest {values['{{VERSION}}']}")


if __name__ == "__main__":
    main()
