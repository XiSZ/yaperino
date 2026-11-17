"""Saves this repository's traffic and release download counts as CSV files.

Run once a day by .github/workflows/traffic.yml, which keeps the files on the
"traffic-data" branch. GitHub itself only keeps traffic for 14 days; merging the
daily numbers here keeps them for good.

Environment: REPO (owner/name), DATA_DIR (where the CSV files go), GH_TOKEN (the
workflow's own token, for releases) and TRAFFIC_TOKEN (a fine-grained token with
"Administration: Read-only" on this repository; without it only downloads are saved).
"""

import csv
import datetime
import json
import os
import urllib.request

API = "https://api.github.com"
REPO = os.environ["REPO"]
DATA = os.environ.get("DATA_DIR", "data")
TODAY = datetime.datetime.now(datetime.timezone.utc).date().isoformat()

ABOUT = """# Yaperino traffic

Saved once a day by `.github/workflows/traffic.yml` on the main branch. GitHub only keeps
traffic for 14 days; these files keep it for good. Times are UTC.

| File | Columns | What |
|---|---|---|
| `views.csv` | date, views, unique | visits to the repository's pages, per day |
| `clones.csv` | date, clones, unique | downloads of the code with git, per day (GitHub's own builds count too) |
| `referrers.csv` | saved_on, site, views, unique | sites visitors came from (GitHub's 14-day totals on that day) |
| `paths.csv` | saved_on, path, title, views, unique | most visited pages (GitHub's 14-day totals on that day) |
| `downloads.csv` | saved_on, release, file, downloads | downloads of each release file so far |
"""


def get(path, token):
    request = urllib.request.Request(
        f"{API}/repos/{REPO}/{path}",
        headers={
            "Authorization": f"Bearer {token}",
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
        },
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.load(response)


def read(name):
    path = os.path.join(DATA, name)
    if not os.path.exists(path):
        return []
    with open(path, newline="", encoding="utf-8") as f:
        return list(csv.reader(f))[1:]


def write(name, header, rows):
    with open(os.path.join(DATA, name), "w", newline="", encoding="utf-8") as f:
        out = csv.writer(f)
        out.writerow(header)
        out.writerows(rows)


def merge_days(name, answer, key):
    """One row per day. Days GitHub still reports replace the saved ones (today's
    numbers may have grown since the last run)."""
    rows = {row[0]: row for row in read(name)}
    for day in answer[key]:
        date = day["timestamp"][:10]
        rows[date] = [date, str(day["count"]), str(day["uniques"])]
    write(name, ["date", key, "unique"], [rows[d] for d in sorted(rows)])


def snapshot(name, header, new_rows):
    """Today's rows replace any saved earlier today; older days stay."""
    rows = [row for row in read(name) if row and row[0] != TODAY]
    write(name, header, rows + new_rows)


def main():
    os.makedirs(DATA, exist_ok=True)
    with open(os.path.join(DATA, "README.md"), "w", encoding="utf-8") as f:
        f.write(ABOUT)

    token = os.environ.get("TRAFFIC_TOKEN", "").strip()
    if token:
        merge_days("views.csv", get("traffic/views?per=day", token), "views")
        merge_days("clones.csv", get("traffic/clones?per=day", token), "clones")
        snapshot(
            "referrers.csv",
            ["saved_on", "site", "views", "unique"],
            [
                [TODAY, r["referrer"], str(r["count"]), str(r["uniques"])]
                for r in get("traffic/popular/referrers", token)
            ],
        )
        snapshot(
            "paths.csv",
            ["saved_on", "path", "title", "views", "unique"],
            [
                [TODAY, p["path"], p["title"], str(p["count"]), str(p["uniques"])]
                for p in get("traffic/popular/paths", token)
            ],
        )
    else:
        print(
            "::warning::The TRAFFIC_TOKEN secret isn't set, so only download counts "
            "were saved. See .github/workflows/traffic.yml."
        )

    releases = get("releases?per_page=100", os.environ["GH_TOKEN"])
    snapshot(
        "downloads.csv",
        ["saved_on", "release", "file", "downloads"],
        [
            [TODAY, release["tag_name"], asset["name"], str(asset["download_count"])]
            for release in releases
            for asset in release["assets"]
        ],
    )


if __name__ == "__main__":
    main()
