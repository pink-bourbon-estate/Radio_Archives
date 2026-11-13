# Roadmap

The plan, in the order it should happen. Nothing here needs ads, trackers or a paid server.

## 1. Stations

- [x] BBC Radio 6 Music (copied here; the original repository still collects the data).
- [x] **Jazz FM** (collector, archive and site are in). Approach changed on 13 Nov 2026: it
  now reads the JSON play-history API directly (about a week of history per request series)
  instead of the playlist page, which only ever held the latest 5 plays. One weekly run fills
  the archive with no gaps, so nothing needs to run every 15 minutes. The decision, the
  `robots.txt` position and the justification are in [JAZZ-FM.md](JAZZ-FM.md). The show is
  derived from the weekly schedule by time. Still worth doing: ask Bauer Media for permission
  or an official feed.

## 2. One catalogue of artists and tracks

**Started** in `catalogue/`: IDs come from the normalised name (no lookup needed), stored as
`artists.tsv` and `tracks.tsv`. Still to do: move 6 Music onto it, and fill in MusicBrainz IDs
and ISRC codes lazily. The text below is the original plan; the one change is that IDs are always
name-based and MusicBrainz IDs are attributes, so an ID never changes once issued.

Today each station keeps its own artist and track lists, so a song played on two stations is
stored twice and cannot be compared. Plan: one shared catalogue that every station references
by ID.

- **Artist ID**: the MusicBrainz artist ID when there is one, otherwise a stable ID made from
  the normalised name.
- **Track ID**: made from the artist ID plus the normalised title (and the MusicBrainz
  recording ID once known).
- Stations store only `(time, show, track ID)`; names, genres and links live once, in the
  catalogue. This also makes "played on both Jazz FM and 6 Music" a cheap query.
- The catalogue is where **ISRC codes** will live (see 3), looked up lazily from MusicBrainz
  for the tracks someone actually wants to export, because the MusicBrainz API is rate-limited.

## 3. Listening: from a list to a playlist

The archives only hold metadata, so a track cannot be played from the site. Today every track
has search links (YouTube, Spotify, SoundCloud, Bandcamp) and every table can be downloaded as
CSV. The next step is turning a show, a year or a chart into a playlist in the user's own
account, the way playlist-transfer tools do:

1. **Match** each track on the destination service. Transfer tools try the ISRC first (an exact
   hit is a certain match), then fall back to searching by title and artist and picking the best
   of the first results. Not every service lets apps search by ISRC.
2. **Create** the playlist through the service's own API, after the user signs in with that
   service (OAuth).
3. **Report** what matched, what matched with low confidence, and what is missing, so nothing is
   silently dropped.

Design rules for ours: it runs entirely in the visitor's browser (OAuth with PKCE, which needs
no secret and no server), nothing is stored, and no tracker is added. It needs one registered
application per service (Spotify first), which the project owner has to create.
Our plays carry artist and title but no ISRC, so step 1 depends on the catalogue (section 2).

## 4. A player (long term)

A downloadable Rust program that plays the music from the catalogue: no ads, no tracking, no
accounts. The archives hold metadata only, so the player has to get the audio from the
listener's own sources (their local files, or their own streaming account). It would use the
same catalogue and the same matching as section 3. A hand-off to a score or piano-learning
app can be built on the MusicBrainz IDs.

## Principles

Open source. No ads. No tracking. Readable files. Unofficial and non-commercial.
