# Radio Archives

Open archives of what radio stations play. Free, readable, no ads, no tracking.

| Station | Site | Status |
|---|---|---|
| BBC Radio 6 Music | `/bbc-radio-6-music/` | Live as a copy. The original, still the main one, is [BBC-Radio-6-Music-Collection](https://github.com/pink-bourbon-estate/BBC-Radio-6-Music-Collection) |
| Jazz FM | `/jazz-fm/` | Next (see [docs/ROADMAP.md](docs/ROADMAP.md)) |

## How it fits together

```
stations/<station>/     one folder per station: its own code, and how its data is collected
landing/                the page that lists the stations
.github/workflows/      builds every station and publishes them as one GitHub Pages site
docs/                   the roadmap and design notes
```

- **BBC Radio 6 Music** is built here from the *data folder* of its original repository
  (checked out read-only on every build), so there is only one scraper and no duplicated data.
  This copy is marked `noindex` so it does not compete with the original in search results.
  When the original is retired, move the data here and remove that flag.
- The site is plain HTML, CSS and JavaScript plus pre-built JSON. It sets no cookies and
  loads nothing from other sites.

Unofficial and non-commercial. Not made, endorsed or checked by any station.
