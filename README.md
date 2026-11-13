# Radio Archives

Open archives of what radio stations play. Free, readable, no ads, no tracking.

| Station | Site | Status |
|---|---|---|
| BBC Radio 6 Music | `/bbc-radio-6-music/` | Live as a copy. The original, still the main one, is [BBC-Radio-6-Music-Collection](https://github.com/pink-bourbon-estate/BBC-Radio-6-Music-Collection) |
| Jazz FM | `/jazz-fm/` | Collecting every 15 minutes on GitHub (the page shows only the latest 5 plays, so there can be gaps; no history before 13 Nov 2026). Site is `noindex` |

## How it fits together

```
stations/<station>/     one folder per station: its own code, and how its data is collected
catalogue/              the shared catalogue of artists and tracks (IDs), used by every station
landing/                the page that lists the stations
.github/workflows/      builds every station and publishes them as one GitHub Pages site
docs/                   the roadmap and design notes
```

- **BBC Radio 6 Music** is built here from the *data folder* of its original repository
  (checked out read-only on every build), so there is only one scraper and no duplicated data.
  This copy is marked `noindex` so it does not compete with the original in search results.
  When the original is retired, move the data here and remove that flag.
- **The shared catalogue** (`catalogue/`) stores each artist and track once, under an ID made
  from the normalised name ("The Beatles" and "Beatles", "Song" and "Song (Remastered 2011)"
  get the same ID). Stations store only `(time, track ID)`. MusicBrainz IDs and ISRC codes are
  attributes added later, never part of the ID. Jazz FM uses it now; 6 Music will move over.
- **Jazz FM** (`stations/jazz-fm/`) reads the public playlist page, checks
  `robots.txt` first, and identifies itself. The page embeds only the latest 5 plays, so a
  complete archive needs frequent runs (see the note in the workflow). Run `cargo run -p jazzfm -- scrape` to collect.
- The site is plain HTML, CSS and JavaScript plus pre-built JSON. It sets no cookies and
  loads nothing from other sites.

Unofficial and non-commercial. Not made, endorsed or checked by any station.
