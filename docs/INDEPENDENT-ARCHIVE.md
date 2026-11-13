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
