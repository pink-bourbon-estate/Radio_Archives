# Radio Archives

Open archives of what radio stations play. Free, readable, no ads, no tracking.

| Station | Site | Status |
|---|---|---|
| BBC Radio 6 Music | `/bbc-radio-6-music/` | Live as a copy. The original, still the main one, is [BBC-Radio-6-Music-Collection](https://github.com/pink-bourbon-estate/BBC-Radio-6-Music-Collection) |
| The Independent Archive | `/independent/` | Small UK independent and community stations, how at risk each is, and why their records matter. Site is `noindex` |
| Jazz FM | `/jazz-fm/` | Collected daily from the JSON API behind the station's playlist page; history from 6 Nov 2026 (the first week was typed in by hand). Site is `noindex` |

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
  attributes, never part of the ID. Jazz FM uses it now; 6 Music will move over.
- **Every artist carries its MusicBrainz ID** wherever one can be found with certainty, and every
  artist page and artist table links to musicbrainz.org. The "MusicBrainz IDs" workflow fills
  them in daily: first from the 6 Music data, then from the MusicBrainz search API (one request
  per 1.1 s, exact and unambiguous matches only; see `tools/musicbrainz.py`).
- **Jazz FM** (`stations/jazz-fm/`) is collected **directly from the JSON API** that the
  station's own playlist page calls when you press "View more":
  `https://listenapi.planetradio.co.uk/api9.2/events/jaz/{cursor}/{count}`. It does not read or
  scrape the web page. Once a day a GitHub workflow asks for the plays since the last run, 48
  per request, walking backwards from now with a 0.25 s pause (about 6 requests, about 17 KB
  a day) and stops as soon as it reaches plays it already has. It identifies itself honestly as
  `RadioArchivesBot` with a link to this repository. **It does not obey the `robots.txt` of
  that API host** (a blanket `Disallow: /`); that is a deliberate decision, explained in full in
  [docs/JAZZ-FM.md](docs/JAZZ-FM.md). Run `cargo run -p jazzfm -- collect` to collect.
- The site is plain HTML, CSS and JavaScript plus pre-built JSON. It sets no cookies and
  loads nothing from other sites.

Unofficial and non-commercial. Not made, endorsed or checked by any station.
