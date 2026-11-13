#!/usr/bin/env python3
"""Collect shows (and tracklists where the uploader added them) of stations that put their shows on Mixcloud.

Reads independent/mixcloud_accounts.tsv (station, Mixcloud account). For each account it asks Mixcloud's public
developer API (api.mixcloud.com, no login) for the account's shows, newest first, then one request per new show for
its detail, which carries the tracklist ("sections") when the uploader entered one. Nothing is downloaded or stored
except metadata: date, title, genre tags, length, link, and the artist/title of each listed track. No audio.
Every show keeps its link back to Mixcloud, and the dashboard links to the station's own website.

api.mixcloud.com/robots.txt disallows bots. The project owner decided (13 Nov 2026) to treat this like Jazz FM
(docs/JAZZ-FM.md): public unauthenticated developer API, tiny load (a few hundred small requests a day, 0.5 s apart),
identified as RadioArchivesBot with a link, non-commercial, removal on request. See docs/INDEPENDENT-ARCHIVE.md.

Writes independent/data/mixcloud/<account>.jsonl, one line per show:
  {"key", "url", "title", "date", "tags", "seconds", "sections": [...raw...]}
Every new show is listed (title, date, tags, link: one request per 100 shows). Only a few shows per account per run
are opened to look for a tracklist, because most small stations do not enter one.
Usage: tools/mixcloud.py [--max-details 400] [--details-per-account 12] [--max-minutes 20]
"""
import argparse, csv, json, os, sys, time, urllib.parse, urllib.request

UA = ("RadioArchivesBot/0.2 (+https://github.com/pink-bourbon-estate/Radio_Archives; "
      "unofficial non-commercial archive; one small batch of requests per day)")
API = "https://api.mixcloud.com"
DELAY = 0.5
DIR = "independent/data/mixcloud"


def get(url):
    time.sleep(DELAY)
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    for attempt in range(3):
        try:
            with urllib.request.urlopen(req, timeout=30) as r:
                return json.load(r)
        except urllib.error.HTTPError as e:
            if e.code in (404, 403):
                return {"error": e.code}
            time.sleep(5 * (attempt + 1))
        except Exception:
            time.sleep(5 * (attempt + 1))
    return {"error": "failed"}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--max-details", type=int, default=400)
    ap.add_argument("--details-per-account", type=int, default=12)
    ap.add_argument("--max-minutes", type=float, default=20)
    a = ap.parse_args()
    t0 = time.time()
    os.makedirs(DIR, exist_ok=True)
    accounts = list(csv.DictReader(open("independent/mixcloud_accounts.tsv", encoding="utf-8"), delimiter="\t"))
    used = 0
    stats = []
    for acc in accounts:
        user = acc["mixcloud_user"]
        path = f"{DIR}/{user}.jsonl"
        have = set()
        if os.path.exists(path):
            have = {json.loads(l)["key"] for l in open(path, encoding="utf-8") if l.strip()}
        url = f"{API}/{urllib.parse.quote(user)}/cloudcasts/?limit=100"
        new = withsec = pages = 0
        mine = 0  # detail requests for this account this run
        status = "ok"
        with open(path, "a", encoding="utf-8") as out:
            while url and time.time() - t0 < a.max_minutes * 60 and pages < 80:
                lst = get(url)
                pages += 1
                if "error" in lst:
                    status = f"error {lst['error']}"
                    break
                data = lst.get("data", [])
                fresh = [d for d in data if d["key"] not in have]
                for d in fresh:
                    if time.time() - t0 > a.max_minutes * 60:
                        break
                    secs = None  # None = detail not looked at; [] = looked at, no tracklist
                    if mine < a.details_per_account and used < a.max_details:
                        det = get(API + d["key"])
                        if "error" not in det:
                            secs = det.get("sections") or []
                            mine += 1
                            used += 1
                    rec = {"key": d["key"], "url": d.get("url", ""), "title": d.get("name", ""),
                           "date": (d.get("created_time") or "")[:10], "tags": [t.get("name", "") for t in d.get("tags", [])],
                           "seconds": d.get("audio_length", 0), "sections": secs}
                    out.write(json.dumps(rec, ensure_ascii=False) + "\n")
                    out.flush()
                    have.add(d["key"])
                    new += 1
                    withsec += 1 if secs else 0
                if not fresh and pages > 1:
                    break  # this whole page was already stored; older pages will be too
                url = (lst.get("paging") or {}).get("next")
        stats.append((acc["station"], user, status, new, withsec))
        print(f"{acc['station']} ({user}): {status}, {new} new shows listed, {mine} looked at in detail, {withsec} with tracklists", flush=True)
    print(f"{used} show details requested in {time.time() - t0:.0f}s")


if __name__ == "__main__":
    main()
