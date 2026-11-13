#!/usr/bin/env python3
"""One-off exploration: find each Mixcloud station's account name and save a small sample of what the
public Mixcloud API returns for it (shows list and one show's detail), to design the collector.
Writes independent/data/mixcloud/_explore.json. Same polite rules as the other collectors."""
import collections, csv, json, os, re, sys, time, urllib.request

UA = ("RadioArchivesBot/0.2 (+https://github.com/pink-bourbon-estate/Radio_Archives; "
      "unofficial non-commercial archive; one small batch of requests per day)")
RESERVED = {"widget", "discover", "upload", "pro", "about", "categories", "popular", "select", "live", "tag",
            "static", "settings", "oembed", "player", "signup", "login", "sitemaps"}


def get(url, text=False):
    time.sleep(0.6)
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            b = r.read(1_500_000).decode("utf-8", "replace")
            return b if text else json.loads(b)
    except Exception as e:
        return None if text else {"error": str(e)}


def user_of(site):
    html = get(f"https://{site.rstrip('/')}/", text=True) or ""
    c = collections.Counter(m.group(1) for m in re.finditer(r"mixcloud\.com/([A-Za-z0-9_.-]+)/", html) if m.group(1).lower() not in RESERVED)
    return c.most_common(1)[0][0] if c else ""


def main():
    rows = list(csv.DictReader(open("independent/stations.tsv", encoding="utf-8"), delimiter="\t"))
    out = {}
    done = 0
    for r in rows:
        if "Mixcloud" not in r.get("track_data", "") or not r.get("website"):
            continue
        u = user_of(r["website"])
        entry = {"user": u}
        if u:
            lst = get(f"https://api.mixcloud.com/{u}/cloudcasts/?limit=20")
            entry["list_keys"] = list(lst.keys()) if isinstance(lst, dict) else str(type(lst))
            data = lst.get("data", []) if isinstance(lst, dict) else []
            entry["count_first_page"] = len(data)
            entry["paging"] = lst.get("paging") if isinstance(lst, dict) else None
            entry["first_item"] = data[0] if data else lst
            if data and done < 3:
                detail = get("https://api.mixcloud.com" + data[0]["key"])
                entry["detail"] = detail
                done += 1
        out[r["station"]] = entry
        print(r["station"], u, entry.get("count_first_page"), flush=True)
    os.makedirs("independent/data/mixcloud", exist_ok=True)
    json.dump(out, open("independent/data/mixcloud/_explore.json", "w"), indent=1, ensure_ascii=False)


if __name__ == "__main__":
    main()
