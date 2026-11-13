# The Independent Archive (plan)

One site that tracks what many small independent and community stations play, starting with
Bristol and growing outwards. Same principles as the rest of this repository: free, no ads, no
tracking, unofficial and non-commercial.

## Registry

`independent/stations.tsv` lists each station: city, kind, music, status, **risk**, website,
contact email, `track_data` (what we know about its public play records), notes and the
source of each fact. Risk is one of:

- `closed` / `paused`: off air. Nothing can be collected live; surviving show tracklists
  (for example on Mixcloud) could still be imported by hand.
- `high`: a public emergency fundraiser, eviction or funding cut in the last few years.
- `watch`: small and self-funded, with no specific public warning found.
- `low`: larger and better funded.
- `unknown`: not confirmed to be on air. Every row starts at
`unchecked`. A station only gets collected once someone has looked at how it publishes tracks.

Places to grow the list: Ofcom's list of community radio stations, the Community Radio Index
(about 300 stations worldwide, launched July 2026) and the Underground Radio Directory.

## Why now

Since 2022 many UK independent stations have closed or cut back, almost always citing rent,
energy and running costs: SWU.FM (Bristol, 2022, later bought by Rinse), 1020 Radio (Bristol,
2023), Melodic Distraction (Liverpool, 2023), Worldwide FM (much reduced, 2022), Vandelay Radio
(Swansea, paused 2024), Coast Radio (Peterhead, 2025), Tone FM (Taunton, 2025). Others have run
emergency fundraisers: Reprezent, Brixton Radio, Threads, Loose.FM, Clyde Built Radio and
Resonance FM. Government money (the Community Radio Fund, about £0.9m in 2026/27) is only for
Ofcom-licensed stations, so online-only stations, which are most of this list, cannot apply.
When a station closes its play history usually goes with it.

## Where a station's track records might come from

1. **A now-playing feed.** Many stations expose the current track (from their streaming
   software or the stream's metadata). That gives only the present moment, so it must be polled
   often to build history. This is the hard case, and the heaviest on a small station's server.
2. **A history or playlist page or API**, like Jazz FM's. One cheap request fetches the past.
3. **Show archives with tracklists** (the station's own site, or Mixcloud). Per-show lists,
   not a minute-by-minute log.
4. **Nothing public.** Then the options are asking the station to send a log, or volunteers
   typing tracklists in, as was done for Jazz FM's first week.

## One site, many stations

The shared catalogue (`catalogue/`) already stores artists and tracks once, so a band played on
two stations is one entry. Each station keeps its own folder under `stations/`, a brand file, and
a collector suited to how it publishes. The existing site generator builds a page per station,
and a combined view answers questions like "which local bands got played this week, and where?".
Local bands are the point, so the catalogue should record an artist's home town when a station
or the artist supplies it.

## Artists: always with their MusicBrainz ID

Every artist any station plays goes into the shared catalogue, and every catalogue artist gets
its MusicBrainz artist ID as soon as one can be matched with certainty (`tools/musicbrainz.py`,
run daily). Pages always show the ID and a link to musicbrainz.org. This matters most for small
local artists: a MusicBrainz ID is a stable, shared reference that outlives any one station's
website, and anyone can add a missing artist to MusicBrainz so the archive can link them.

## The rule (decided by the project owner, 13 Nov 2026)

Independent and community stations are closing, and an archive is worth more the sooner it
starts, so collection does not wait for replies. For every station with a website:

1. **Check `robots.txt` first** (`tools/survey.py`, run by the "Independent stations survey"
   workflow). The result is stored in `independent/stations.tsv`:
   `allowed`, `blocked`, `none` (no robots.txt) or `unknown`.
2. **Allowed or none: collect.** Use the station's public feed or pages, politely.
3. **Blocked: mark it and find a contact email** (stored as `contact_email`), then collect the
   same way as Jazz FM (see `docs/JAZZ-FM.md`): the lightest possible requests, honestly
   identified as `RadioArchivesBot`, and a plain statement that the station's `robots.txt` is
   not followed.
4. **Once every station has been surveyed, send one blanket email** to all of them, saying what
   the project is, what is being collected and how to stop it, and go from there.

Limits that apply to every station, whatever the survey says:
- Requests are spread out and as few as possible; small volunteer servers are never hammered.
- A request to stop or to remove a station's data is honoured immediately, and the station is
  marked `opted_out` in the registry.
- The bot always identifies itself and links to this repository.

## Registry columns added by the survey

`robots` (allowed / blocked / none / unknown), `contact_email`, `checked_on`. A station with no
website yet is skipped until one is added.

## Next steps

1. Confirm which Bristol stations are still on air and find websites for the blanks, then run the survey workflow.
2. For each station, check how it publishes tracks (the four cases above) and update `track_data`.
3. Ask the friend which station they meant, so it can be surveyed first.
4. Build the first collector for the station with the cleanest data.


## The dashboard and how stations feed it

`/independent/` is the same dashboard as the 6 Music and Jazz FM archives (built by the same
generator, `bbc6 site`, with `independent/site/brand.json` and `about.html`). Every programme is
shown as "Station: programme", so one dashboard covers all stations. `/independent/stations/` is
the list of stations and the article.

* `tools/noods.py` (daily, `independent-collect.yml`) collects Noods Radio from its public JSON feed
  (`panel.noodsradio.com`: residents, paginated shows, per-show tracklists). robots.txt allows everything.
  Up to 400 shows a run, 0.4 s between requests, identified as RadioArchivesBot. Data goes to
  `independent/data/noods/shows.jsonl`. SoundCloud was considered and not used: the feed has the full
  history with structured tracklists already.
* `tools/independent_export.py` turns the collected shows into the shared plays CSV format.
* Artists are listed in `independent/data/artists.tsv` and get MusicBrainz IDs from `tools/musicbrainz.py`
  (same exact-match rule as Jazz FM).
* Next stations: check each one's robots.txt, find how it publishes tracks, and add a collector.

## What the stations publish (probe, 13 Nov 2026)

`tools/probe.py` (workflow "Independent stations probe") looked at 55 stations whose robots.txt allows
us. Results are in `independent/probe.tsv`; `track_data` in `stations.tsv` summarises them.

* Almost no small station publishes a track log of its own. Noods Radio is the exception (full tracklists).
* Most put their shows on Mixcloud. Mixcloud's public API could give programme lists (and tracklists where
  uploaders added them) for dozens of stations with one collector, but `api.mixcloud.com/robots.txt`
  disallows bots, so by the project rule it is marked and waits for a decision (the Jazz FM route, or not).
* A few have playlist pages worth inspecting: Shoreditch Radio, One Jazz, Skylab Radio, Fuse FM,
  Soundart Radio, Rinse FM, Margate Radio.
* Some sites answer 200 for any path, so "/playlist exists" in the probe is not proof of a playlist.
* Ofcom's community radio list page (static.ofcom.org.uk) disallows bots, so it is not used.

## Principles (from the project owner, 13 Nov 2026)

* No songs, recordings or other works are taken. Only listing facts (date, programme, artist, title) and links.
* No monetisation of any kind: no adverts, sponsorship, sales, donations, paid tiers or tracking.
* Every programme links to the station's own website and, where used, its SoundCloud or Mixcloud page, so listeners are sent to the station.
* The reason the project exists is the article "Keeping a record of small radio before it disappears" on the Pink Bourbon Estate site.
* Mixcloud's API robots.txt disallows bots; by the project rule it is collected the Jazz FM way (public, unauthenticated, tiny, identified, removal on request).

## Revised brief (13 Nov 2026): a directory and a programme record, with tracks where they exist

What the stations publish (see "What the stations publish" above) means a track-level dashboard is only possible for a
few (Noods Radio). The project is therefore broadened, and says so plainly on the site:

* **A station directory:** name, town/city, website, stream/player, status (on air, at risk, closed), and where each
  station's shows are found (Mixcloud, SoundCloud, its own site). Stations are found from public lists and from
  Radioplayer's station search (its robots.txt disallows /api/; collected the Jazz FM way, one small pass, identified).
* **A programme record:** the weekly timetable of each station (most publish one; about one small request a week per
  station), the programmes and presenters, and guest artists named on show pages, with links to the station.
* **Tracks only where a station publishes them** (Noods now; others if they start). If a station begins publishing
  tracklists later, it already has a record to grow from.
* **Said openly on the site:** because small stations rarely publish tracklists, this archive cannot list the tracks they
  play, except where noted. It records who is on air, when and where, so the culture of independent radio is not lost.
* Guest and artist names (e.g. Radio Buena Vida's guests, Resonance Extra and Totally Radio episode pages) can help with
  obscure artists; record shops and venues that run radio events (e.g. Phonica Records) are a possible extra source.
* Pages built in the browser (ROVR, Resonance Extra, Totally Radio) need a headless browser to read.
