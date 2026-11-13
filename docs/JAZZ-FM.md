# Jazz FM: how the archive is built

This is the single, authoritative description of how the Jazz FM archive is collected. It was
decided by the project owner on 13 November 2026 and replaces the earlier approach.

## The approach

Jazz FM's public playlist page (`https://www.hellorayo.co.uk/jazz-fm/playlist`) shows the latest
few plays and a "View more" button. That button calls a public JSON endpoint:

```
GET https://listenapi.planetradio.co.uk/api9.2/events/jaz/{cursor}/{count}
```

- `jaz` is the station code for Jazz FM.
- `cursor` is a UK local time, `YYYY-MM-DD HH:MM:SS`, URL-encoded. The reply is the `count`
  plays played immediately before it, newest first.
- Each item has `nowPlayingArtist`, `nowPlayingTrack` and `nowPlayingTime` (UK local time, no
  time zone, converted to UTC here).
- To go further back, the oldest time in a reply minus one second is the next cursor.

**We call this API directly. We do not load, render or scrape the web page.**

Once a day (05:17 UTC, `.github/workflows/jazz-fm.yml`) `jazzfm collect`:

1. Finds the newest play already stored.
2. Walks backwards from now, 48 plays per request, pausing 0.25 s between requests.
3. Stops as soon as it reaches plays that are already stored (with one hour of overlap), when
   the API has no earlier plays, or at a hard limit of 400 requests.
4. Adds the new plays to `stations/jazz-fm/data/plays/YYYY-MM.tsv` and the shared catalogue.

A day of history is about 6 requests of about 2.8 KB each, roughly 17 KB in total, taking
about 2 seconds (about 42 requests and 120 KB a week). The API keeps roughly a week or more of
history, so each run overlaps the previous one, and a missed run is filled in by the next one:
there are about six runs of slack before anything could be lost. Each run that finds new plays
makes one commit.

It identifies itself as `RadioArchivesBot/0.2 (+https://github.com/pink-bourbon-estate/Radio_Archives; ...)`.
It does not send browser headers (no browser `User-Agent`, `Origin` or `Referer`) and does not
pretend to be the website.

## robots.txt: a deliberate decision

`listenapi.planetradio.co.uk` serves a `robots.txt` that disallows automated access. This
project **does not obey it for this host**, and says so openly. The reasons:

1. **What the file is for.** API hosts almost universally publish a blanket `Disallow: /` so
   that search engines do not index raw JSON endpoints in place of the real web pages. It is not
   a statement that the play list is secret. The same data is shown to every visitor of the
   public playlist page. The endpoint is unauthenticated and publicly cacheable (its responses
   carry `cache-control: max-age=3600, public` and are served through Cloudflare).
2. **Load.** About 42 small JSON requests a week (6 a day), about 120 KB in total, 0.25 s
   apart, mostly answered from cache. It never loads the web page, its scripts or its styles,
   and puts close to no load on the origin.
3. **Nature of the data.** There is no paywall or login to bypass, no personal data (it is song
   titles, artist names and broadcast times), no degradation of the service (about 2 seconds
   of requests, once a day), and no commercial redistribution. It is a non-commercial archive
   for personal listening and research.
4. **Honesty.** The decision is stated in the README, on the site's About page and here, rather
   than hidden. Anyone can see exactly what the program does by reading `stations/jazz-fm/src/api.rs`.

This is a judgement about intent and impact, not a claim that robots.txt does not apply: the
file asks automated programs to stay out and this project chooses not to. The standing remedy is
removal on request: if Jazz FM, Bauer Media, an artist or a rights holder asks, the collector is
switched off and anything asked for is removed (issues on this repository). The better long-term
route, still on the roadmap, is to ask Bauer Media for permission or an official feed.

## What the data is, and is not

- Artist, title and play time. No audio, artwork, album, genre or ISRC.
- Each artist's **MusicBrainz ID** is added by the daily "MusicBrainz IDs" workflow
  (`tools/musicbrainz.py`): from the 6 Music data where the same artist appears there, otherwise
  from an exact, unambiguous MusicBrainz search match. Joint credits such as "A and B" are left
  without an ID rather than guessed. The site shows the ID and a MusicBrainz link on every artist.
- The API does not say which show was on; the show is derived from Jazz FM's usual weekly
  timetable (`stations/jazz-fm/src/show.rs`) and can be wrong for special programmes.
- The first week (6 to 13 November 2026) was collected by hand and typed in by screenshot
  (`data/imports/`), so a few names may contain mistakes and a few short stretches are missing.
- Tracks are matched by normalised artist and title, so different versions can be counted together.

## Code map

| File | Role |
|---|---|
| `stations/jazz-fm/src/api.rs` | the request loop: cursor, batch size, pause, stopping rules |
| `stations/jazz-fm/src/extract.rs` | reading a JSON reply into plays; UK local time to UTC |
| `stations/jazz-fm/src/store.rs` | monthly `plays/YYYY-MM.tsv` files, merged and de-duplicated |
| `stations/jazz-fm/src/show.rs` | the show for a time, from the weekly timetable |
| `.github/workflows/jazz-fm.yml` | the daily run |

Commands: `jazzfm collect [--days N] [--json FILE]`, `import`, `export`, `stats`.
`--json FILE` reads a saved API reply instead of calling the network, for testing.
