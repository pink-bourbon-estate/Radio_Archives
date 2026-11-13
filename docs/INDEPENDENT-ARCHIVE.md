# The Independent Archive (plan)

One site that tracks what many small independent and community stations play, starting with
Bristol and growing outwards. Same principles as the rest of this repository: free, no ads, no
tracking, unofficial and non-commercial.

## Registry

`independent/stations.tsv` lists each station: area, kind, music, status, website, and
`track_data`, which says what we know about its public play records. Every row starts at
`unchecked`. A station only gets collected once someone has looked at how it publishes tracks.

Places to grow the list: Ofcom's list of community radio stations, the Community Radio Index
(about 300 stations worldwide, launched July 2026) and the Underground Radio Directory.

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

## A different rule for small stations

Jazz FM is a large commercial network, and its collector is documented in `docs/JAZZ-FM.md`.
Small volunteer stations are not the same: their servers are small and their goodwill is the
whole project. Recommended for the Independent Archive:

- **Ask first.** Contact each station, explain the project, and collect only with a yes.
- **Prefer what they offer**: a log export, a feed they point us to, or a share of their data.
- **Poll gently** and follow their robots.txt, since a friendly relationship is worth more than
  any dataset.
- Provide a clear way to be removed.

## Next steps

1. Confirm which Bristol stations are still on air and find websites for the blanks.
2. For each, check how it publishes tracks (the four cases above) and update `track_data`.
3. Ask the friend which station they meant, and ask that station first.
4. Build the first collector for the station with the cleanest data.
