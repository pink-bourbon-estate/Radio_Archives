#!/usr/bin/env python3
"""Fill in MusicBrainz artist IDs in the shared catalogue (catalogue/data/artists.tsv).

Every station that uses the catalogue (Jazz FM now, Independent Archive stations later) then
shows each artist's MusicBrainz ID and a link to musicbrainz.org.

Two steps, cheapest first:
  1. From the 6 Music data (--bbc6 DIR), which already carries a MusicBrainz artist ID on most
     plays. An artist gets that ID only when every 6 Music play under the same normalised name
     agrees on one ID. No network needed.
  2. The MusicBrainz search API for the rest, at most one request every 1.1 seconds (their
     published limit is one per second) with a User-Agent that names this project. An ID is
     only accepted when the best result scores 100, its name (or one of its aliases) equals
     ours after normalising, and no other result with the same name scores 90 or more. Anything
     less certain is recorded as "ambiguous" or "none" and left empty: a wrong ID is worse than
     no ID. Those are tried again after 90 days.

musicbrainz.org's robots.txt disallows /ws/ (its API) for crawlers, while its API documentation
invites programs to use it at one request per second with an identifying User-Agent. This tool
follows the API rules exactly; the decision is the same as for Jazz FM (docs/JAZZ-FM.md) and is
stated in the README.

Every lookup is written to catalogue/data/musicbrainz_lookups.tsv so it is never repeated
needlessly. Usage: tools/musicbrainz.py [--bbc6 DIR] [--max N] [--offline] [--artists FILE --log FILE]
"""
import csv, glob, json, os, re, sys, time, unicodedata, urllib.parse, urllib.request, urllib.robotparser
from datetime import date, timedelta

CAT = "catalogue/data/artists.tsv"
LOG = "catalogue/data/musicbrainz_lookups.tsv"
API = "https://musicbrainz.org/ws/2/artist/"
UA = "RadioArchives/0.2 ( https://github.com/pink-bourbon-estate/Radio_Archives )"
GAP = 1.1
RETRY_DAYS = 90
LOG_COLS = ["artist_id", "name", "result", "mbid", "score", "checked_on"]


def norm(s):
    s = unicodedata.normalize("NFKD", s).encode("ascii", "ignore").decode().lower()
    s = s.replace("&", " and ")
    s = re.sub(r"^the\s+", "", s.strip())
    s = re.sub(r"[^a-z0-9]+", " ", s)
    return " ".join(s.split())


def read_tsv(path):
    if not os.path.exists(path):
        return []
    with open(path, newline="", encoding="utf-8") as f:
        return list(csv.DictReader(f, delimiter="\t", quoting=csv.QUOTE_NONE))


def write_tsv(path, rows, cols):
    with open(path, "w", newline="", encoding="utf-8") as f:
        w = csv.DictWriter(f, fieldnames=cols, delimiter="\t", lineterminator="\n",
                           quoting=csv.QUOTE_NONE, escapechar=None, extrasaction="ignore")
        w.writeheader()
        w.writerows(rows)


def from_bbc6(folder):
    """normalised name -> the single MusicBrainz ID 6 Music uses for it (skipped if they disagree)."""
    seen = {}
    files = glob.glob(os.path.join(folder, "**", "*.csv"), recursive=True)
    for p in files:
        with open(p, newline="", encoding="utf-8", errors="replace") as f:
            r = csv.DictReader(f)
            if not r.fieldnames or "Artist" not in r.fieldnames or "MusicBrainzArtistID" not in r.fieldnames:
                continue
            for row in r:
                m = (row.get("MusicBrainzArtistID") or "").strip()
                if re.fullmatch(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}", m):
                    seen.setdefault(norm(row.get("Artist") or ""), set()).add(m)
    return {k: next(iter(v)) for k, v in seen.items() if k and len(v) == 1}


def choose(name, results):
    """(result, mbid, score) from a MusicBrainz search reply, by the strict rule above."""
    n = norm(name)

    def names(a):
        return {norm(a.get("name", ""))} | {norm(x.get("name", "")) for x in a.get("aliases", []) or []}

    same = [a for a in results if n in names(a)]
    if not same:
        return "none", "", results[0].get("score", 0) if results else 0
    best = max(same, key=lambda a: a.get("score", 0))
    rivals = [a for a in same if a is not best and a.get("score", 0) >= 90]
    if best.get("score", 0) < 100 or rivals:
        return "ambiguous", "", best.get("score", 0)
    return "found", best["id"], 100


def search(name):
    q = urllib.parse.urlencode({"query": f'artist:"{name}"', "fmt": "json", "limit": "5"})
    req = urllib.request.Request(f"{API}?{q}", headers={"User-Agent": UA, "Accept": "application/json"})
    for wait in (10, 20, 40, 60, 90):
        try:
            with urllib.request.urlopen(req, timeout=30) as r:
                return json.load(r).get("artists", [])
        except urllib.error.HTTPError as e:
            if e.code in (503, 429):  # MusicBrainz's "slow down": back off and try again
                time.sleep(wait)
                continue
            raise
    raise RuntimeError("MusicBrainz kept answering 'slow down'")


def main(argv):
    global CAT, LOG
    if "--artists" in argv:  # another artist list with the same columns, e.g. the Independent Archive's
        CAT = argv[argv.index("--artists") + 1]
    if "--log" in argv:
        LOG = argv[argv.index("--log") + 1]
    bbc6 = argv[argv.index("--bbc6") + 1] if "--bbc6" in argv else None
    limit = int(argv[argv.index("--max") + 1]) if "--max" in argv else 2000
    offline = "--offline" in argv
    with open(CAT, encoding="utf-8") as f:
        header, *lines = f.read().splitlines()
    artists = [dict(zip(["id", "name", "mbid"], (l.split("\t") + ["", ""])[:3])) for l in lines if l]
    log = {r["artist_id"]: r for r in read_tsv(LOG)}
    today = date.today()
    known = sum(1 for a in artists if a["mbid"])

    if bbc6:
        ids = from_bbc6(bbc6)
        got = 0
        for a in artists:
            if not a["mbid"] and norm(a["name"]) in ids:
                a["mbid"] = ids[norm(a["name"])]
                log[a["id"]] = {"artist_id": a["id"], "name": a["name"], "result": "from 6 Music",
                                "mbid": a["mbid"], "score": "", "checked_on": today.isoformat()}
                got += 1
        print(f"6 Music data: {len(ids)} names with one agreed ID; {got} catalogue artists filled in")

    if not offline:
        rp = urllib.robotparser.RobotFileParser("https://musicbrainz.org/robots.txt")
        try:
            rp.read()
            print("musicbrainz.org robots.txt allows /ws/2/ for us:", rp.can_fetch(UA, API))
        except Exception as e:
            print("could not read musicbrainz.org robots.txt:", e)
        todo = []
        for a in artists:
            if a["mbid"]:
                continue
            prev = log.get(a["id"])
            if prev and prev["checked_on"] and date.fromisoformat(prev["checked_on"]) > today - timedelta(days=RETRY_DAYS):
                continue
            todo.append(a)
        print(f"MusicBrainz: {len(todo)} artists to look up (doing at most {limit})")
        counts = {}
        last = 0.0
        failures = 0
        for a in todo[:limit]:
            wait = GAP - (time.time() - last)
            if wait > 0:
                time.sleep(wait)
            last = time.time()
            try:
                res, mbid, score = choose(a["name"], search(a["name"]))
            except Exception as e:
                failures += 1
                print(f"  {a['name']}: {e}")
                if failures >= 5:
                    print("  five failures in a row; stopping until the next run")
                    break
                time.sleep(30)
                continue
            failures = 0
            counts[res] = counts.get(res, 0) + 1
            if mbid:
                a["mbid"] = mbid
            log[a["id"]] = {"artist_id": a["id"], "name": a["name"], "result": res, "mbid": mbid,
                            "score": str(score), "checked_on": today.isoformat()}
        print("MusicBrainz results:", counts)

    with open(CAT, "w", encoding="utf-8") as f:
        f.write(header + "\n" + "".join(f"{a['id']}\t{a['name']}\t{a['mbid']}\n" for a in artists))
    write_tsv(LOG, sorted(log.values(), key=lambda r: r["name"].lower()), LOG_COLS)
    now = sum(1 for a in artists if a["mbid"])
    print(f"artists with a MusicBrainz ID: {known} -> {now} of {len(artists)}")


if __name__ == "__main__":
    main(sys.argv[1:])
