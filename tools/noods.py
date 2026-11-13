#!/usr/bin/env python3
"""Collect Noods Radio (Bristol) shows and tracklists into independent/data/noods/shows.jsonl.

Noods' own website loads everything from a public JSON feed at panel.noodsradio.com
(shows, residents and per-show pages with the tracklist). Its robots.txt is empty (everything
allowed), so this is plain, polite collection: identifying User-Agent, a pause between requests,
and a cap per run. The archive grows over several daily runs until it has caught up.

    python3 tools/noods.py [--max-shows 400] [--delay 0.4]

One line per show: {"slug", "date" (ISO), "title", "start" (HH:MM or ""), "genres", "tracks": [[artist, title], ...]}
Shows with no tracklist are stored too (tracks: []) so they are not asked for again.
"""
import argparse, html, json, os, re, sys, time, urllib.request

BASE = "https://panel.noodsradio.com"
UA = "RadioArchivesBot/0.2 (+https://github.com/pink-bourbon-estate/Radio_Archives; unofficial non-commercial archive)"
OUT = "independent/data/noods/shows.jsonl"


def get(path, delay):
    time.sleep(delay)
    url = path if path.startswith("http") else f"{BASE}/{path.lstrip('/')}"
    req = urllib.request.Request(url, headers={"User-Agent": UA, "Accept": "application/json"})
    for attempt in range(3):
        try:
            with urllib.request.urlopen(req, timeout=30) as r:
                return json.load(r)
        except Exception as e:  # noqa: BLE001
            if attempt == 2:
                print(f"  gave up on {url}: {e}", file=sys.stderr)
                return None
            time.sleep(5 * (attempt + 1))


def resident_ids(listing):
    """Every resident id ('residents/slug') anywhere in residents.json."""
    found = []

    def walk(x):
        if isinstance(x, dict):
            v = x.get("id")
            if isinstance(v, str) and v.startswith("residents/"):
                found.append(v)
            for y in x.values():
                walk(y)
        elif isinstance(x, list):
            for y in x:
                walk(y)
        elif isinstance(x, str) and x.startswith("residents/"):
            found.append(x)

    walk(listing)
    return list(dict.fromkeys(found))


def start_of(residenttime):
    m = re.match(r"\s*(\d{2})(\d{2})\b", residenttime or "")
    return f"{m.group(1)}:{m.group(2)}" if m else ""


def iso(date):  # "12.11.26" -> "2026-11-12"
    m = re.fullmatch(r"(\d\d)\.(\d\d)\.(\d\d)", (date or "").strip())
    return f"20{m.group(3)}-{m.group(2)}-{m.group(1)}" if m else ""


def tracks_of(tracklist_html):
    text = re.sub(r"<br\s*/?>|</p>|</li>", "\n", tracklist_html or "")
    text = html.unescape(re.sub(r"<[^>]+>", "", text))
    out = []
    for line in text.splitlines():
        line = re.sub(r"^\s*(\d+[.)]\s*)?", "", line).strip()
        parts = re.split(r"\s+[-–—]\s+", line, maxsplit=1)
        if len(parts) == 2 and parts[0].strip() and parts[1].strip():
            out.append([parts[0].strip(), parts[1].strip()])
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--max-shows", type=int, default=400)
    ap.add_argument("--delay", type=float, default=0.4)
    ap.add_argument("--max-minutes", type=float, default=20)
    a = ap.parse_args()

    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    have = set()
    if os.path.exists(OUT):
        for line in open(OUT, encoding="utf-8"):
            if line.strip():
                have.add(json.loads(line)["slug"])
    print(f"{len(have)} shows already stored")

    listing = get("residents.json", a.delay)
    if not listing:
        sys.exit("could not read residents.json")
    residents = resident_ids(listing)
    # the latest shows first, whatever the resident
    shows = get("shows.json", a.delay) or {}
    queue = []  # (show id, resident start time)
    for key in ("featured", "latest"):
        for p in shows.get(key, []):
            queue.append((p.get("id", ""), ""))
    print(f"{len(residents)} residents", flush=True)

    new = 0
    t0 = time.time()
    with open(OUT, "a", encoding="utf-8") as out:

        def take(show_id, start):
            nonlocal new
            slug = show_id.split("/", 1)[-1]
            if not show_id.startswith("shows/") or slug in have or new >= a.max_shows or time.time() - t0 > a.max_minutes * 60:
                return
            d = get(f"{show_id}.json", a.delay)
            if not d:
                return
            rec = {
                "slug": slug,
                "date": iso(d.get("date")),
                "title": d.get("title") or "",
                "start": start,
                "genres": d.get("genretags") or d.get("genretag") or [],
                "tracks": tracks_of((d.get("tracklist") or {}).get("html", "")),
            }
            out.write(json.dumps(rec, ensure_ascii=False) + "\n")
            out.flush()
            if new % 25 == 0:
                print(f"  {new} shows so far ({time.time() - t0:.0f}s)", flush=True)
            have.add(slug)
            new += 1

        for sid, start in queue:
            take(sid, start)
        for rid in residents:
            if new >= a.max_shows or time.time() - t0 > a.max_minutes * 60:
                break
            url = f"{rid}.json"
            start = ""
            page = 0
            while url and new < a.max_shows and page < 60 and time.time() - t0 < a.max_minutes * 60:
                r = get(url, a.delay)
                if not r:
                    break
                if page == 0:
                    start = start_of(r.get("residenttime", ""))
                posts = r.get("posts", [])
                for p in posts:
                    take(p.get("id", ""), start)
                if posts and all(p.get("id", "").split("/", 1)[-1] in have for p in posts) and page > 0:
                    break  # this page was all known already; older ones will be too
                pg = r.get("pagination") or {}
                url = pg.get("paginationUrl") if pg.get("hasNextPage") else None
                page += 1
    print(f"added {new} shows; {len(have)} stored")


if __name__ == "__main__":
    main()
