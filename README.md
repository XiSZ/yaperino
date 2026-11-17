# Yaperino traffic

Saved once a day by `.github/workflows/traffic.yml` on the main branch. GitHub only keeps
traffic for 14 days; these files keep it for good. Times are UTC.

| File | Columns | What |
|---|---|---|
| `views.csv` | date, views, unique | visits to the repository's pages, per day |
| `clones.csv` | date, clones, unique | downloads of the code with git, per day (GitHub's own builds count too) |
| `referrers.csv` | saved_on, site, views, unique | sites visitors came from (GitHub's 14-day totals on that day) |
| `paths.csv` | saved_on, path, title, views, unique | most visited pages (GitHub's 14-day totals on that day) |
| `downloads.csv` | saved_on, release, file, downloads | downloads of each release file so far |
