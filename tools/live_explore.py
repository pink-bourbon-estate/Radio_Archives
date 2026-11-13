#!/usr/bin/env python3
"""One-off: look at what the public now-playing feeds of AzuraCast and radio.co stations return, to see whether any keeps a
track history we could collect once a day. Saves a small sample to independent/data/live/_explore.json.
Stations are the ones the probe recognised. robots.txt is checked for the station site; feed endpoints are public player APIs."""
import csv, json, os, re, time, urllib.request, urllib.robotparser

UA = ("RadioArchivesBot/0.2 (+https://github.com/pink-bourbon-estate/Radio_Archives; "
      "unofficial non-commercial archive; a few requests per station)")


def get(url, text=False):
    time.sleep(1.5)
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    try:
        with urllib.request.urlopen(req, timeout=25) as r:
            b = r.read(800_000).decode("utf-8", "replace")
            return b if text else json.loads(b)
    except Exception as e:
        return {"error": str(e)[:100]} if not text else ""


def main():
    rows = list(csv.DictReader(open("independent/probe.tsv", encoding="utf-8"), delimiter="\t"))
    out = {}
    for r in rows:
        site, plat = r["website"].split("/")[0], r["platform"]
        res = {}
        html = get(f"https://{site}/", text=True)
        if "AzuraCast" in plat:
            res["azuracast"] = {}
            for path in ("/api/nowplaying", "/api/nowplaying/1"):
                j = get(f"https://{site}{path}")
                res["azuracast"][path] = j if "error" in j else (j[0] if isinstance(j, list) and j else j)
                if isinstance(res["azuracast"][path], dict) and "song_history" in str(res["azuracast"][path])[:100000]:
                    break
            m = re.findall(r"https?://[^\"'\s]+/api/nowplaying[^\"'\s]*", html)
            res["azuracast_urls_in_page"] = m[:3]
        if "radio.co" in plat:
            ids = set(re.findall(r"public\.radio\.co/stations/([a-z0-9]+)", html)) | set(re.findall(r"s([0-9a-f]{9,10})\.(?:radio\.co|us-east)", html)) | set(re.findall(r"radio\.co/(?:embed/)?(?:stations/)?(s[0-9a-f]{9,10})", html))
            res["radioco_ids"] = sorted(ids)[:3]
            for i in sorted(ids)[:1]:
                res["radioco_status"] = get(f"https://public.radio.co/stations/{i}/status")
        if res:
            out[r["station"]] = res
            print(r["station"], list(res.keys()), flush=True)
    os.makedirs("independent/data/live", exist_ok=True)
    json.dump(out, open("independent/data/live/_explore.json", "w"), indent=1, ensure_ascii=False)


if __name__ == "__main__":
    main()
