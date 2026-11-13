#!/usr/bin/env python3
"""Turn the Independent Archive's collected shows into the format the shared site generator reads.

    python3 tools/independent_export.py OUT_DIR

Reads independent/data/<station>/shows.jsonl (one collector per station; Noods Radio so far) and
writes OUT_DIR/plays/plays_YYYY.csv (the 6 Music / Jazz FM column layout) plus OUT_DIR/plays.csv.
Each programme is shown as "<Station>: <Programme>", so one dashboard covers every station.
Artists are listed in independent/data/artists.tsv (id = normalised name) so tools/musicbrainz.py
can give them a MusicBrainz ID; IDs already known from the Jazz FM catalogue are reused.
"""
import csv, glob, json, os, sys
sys.path.insert(0, os.path.dirname(__file__))
from musicbrainz import norm, read_tsv  # noqa: E402

STATIONS = {"noods": "Noods Radio"}
ARTISTS = "independent/data/artists.tsv"
HEADER = ["Date", "Time", "Show", "Episode", "EpisodePID", "Position", "OffsetSec", "Artist", "Title",
          "Album", "Label", "Publisher", "DurationSec", "MusicBrainzArtistID"]


def main(out):
    plays = []
    for folder, station in STATIONS.items():
        for path in glob.glob(f"independent/data/{folder}/shows.jsonl"):
            for line in open(path, encoding="utf-8"):
                if line.strip():
                    plays.append((station, json.loads(line)))

    known = {}
    for r in read_tsv(ARTISTS):
        known[r["id"]] = r
    for r in read_tsv("catalogue/data/artists.tsv"):
        if r.get("mbid"):
            known.setdefault(norm(r["name"]), r)
    own = {r["id"]: r for r in read_tsv(ARTISTS)}
    for _, show in plays:
        for artist, _t in show["tracks"]:
            k = norm(artist)
            if k and k not in own:
                own[k] = {"id": k, "name": artist, "mbid": known.get(k, {}).get("mbid", "")}
    os.makedirs(os.path.dirname(ARTISTS), exist_ok=True)
    with open(ARTISTS, "w", encoding="utf-8") as f:
        f.write("id\tname\tmbid\n" + "".join(f"{r['id']}\t{r['name'].replace(chr(9), ' ')}\t{r['mbid']}\n"
                                              for r in sorted(own.values(), key=lambda r: r["id"])))

    os.makedirs(os.path.join(out, "plays"), exist_ok=True)
    by_year, plain = {}, [["date", "time", "station", "programme", "artist", "title", "artist_musicbrainz_id"]]
    n = 0
    for station, show in sorted(plays, key=lambda p: (p[1]["date"], p[1]["slug"])):
        date = show["date"]
        if not date:
            continue
        name = f"{station}: {show['title']}"
        time_ = show.get("start") or "00:00"
        episode = f"{date[8:10]}/{date[5:7]}/{date[:4]}"
        for pos, (artist, title) in enumerate(show["tracks"], 1):
            mbid = own.get(norm(artist), {}).get("mbid", "")
            by_year.setdefault(date[:4], []).append(
                [date, time_, name, episode, "", pos, "", artist, title, "", "", "", "", mbid])
            plain.append([date, time_, station, show["title"], artist, title, mbid])
            n += 1
    for year, rows in by_year.items():
        with open(os.path.join(out, "plays", f"plays_{year}.csv"), "w", newline="", encoding="utf-8") as f:
            w = csv.writer(f)
            w.writerow(HEADER)
            w.writerows(rows)
    with open(os.path.join(out, "plays.csv"), "w", newline="", encoding="utf-8") as f:
        csv.writer(f).writerows(plain)
    print(f"{n} plays from {len(plays)} shows, {len(own)} artists")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "_independent_data")
