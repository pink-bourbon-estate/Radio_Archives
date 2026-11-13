#!/usr/bin/env python3
"""Probe each independent station's website for how it publishes tracks and programmes.

Reads independent/stations.tsv; only stations whose robots.txt result is allowed or none are touched.
For each, reads the home page and a handful of usual paths (2 seconds apart, as RadioArchivesBot),
and records in independent/probe.tsv:
  platform   streaming/hosting platforms recognised in the page (radio.co, AzuraCast, Mixcloud, SoundCloud, ...)
  feeds      RSS/Atom links and whether /wp-json/ exists
  tracks     words/links suggesting published track data (tracklist, playlist, recently played, now playing)
  shows      links suggesting programme/schedule/show pages
  status     HTTP status of the home page
Nothing is collected beyond what a visitor would load. Usage: tools/probe.py
"""
import csv, re, sys, time, urllib.error, urllib.parse, urllib.request, urllib.robotparser
from datetime import date

UA = ("RadioArchivesBot/0.2 (+https://github.com/pink-bourbon-estate/Radio_Archives; "
      "unofficial non-commercial archive; a few requests per station)")
PAUSE = 2.0
PLATFORMS = {
    "radio.co": r"radio\.co\b|public\.radio\.co", "AzuraCast": r"azuracast|/api/nowplaying",
    "Radio Cult": r"radiocult", "Mixcloud": r"mixcloud\.com", "SoundCloud": r"soundcloud\.com",
    "Spinitron": r"spinitron", "Zeno": r"zeno\.fm|zenomedia", "Radio King": r"radioking",
    "Radio Jar": r"radiojar", "Live365": r"live365", "Centova": r"centova", "Shoutcast": r"shoutcast",
    "Icecast": r"icecast", "StreamTheWorld": r"streamtheworld|tunein", "Airtime": r"airtime",
    "Caster.fm": r"caster\.fm", "Listen2MyRadio": r"listen2myradio", "WordPress": r"wp-content|wp-json",
    "Wix": r"wixstatic|wix\.com", "Squarespace": r"squarespace", "Bandcamp": r"bandcamp\.com",
    "YouTube": r"youtube\.com|youtu\.be", "Twitch": r"twitch\.tv", "Mixlr": r"mixlr",
    "Podcast host": r"acast\.com|podbean|buzzsprout|libsyn|anchor\.fm|spotify\.com/show",
}
TRACK_WORDS = r"tracklist|track list|playlist|recently played|now playing|last played|song history|played tracks"
SHOW_WORDS = r"schedule|shows|programmes|programs|listen again|archive|on demand|catch up|podcasts?"
PATHS = ["", "playlist", "tracklist", "recently-played", "schedule", "shows", "programmes", "archive", "wp-json/"]


def fetch(url):
    req = urllib.request.Request(url, headers={"User-Agent": UA, "Accept": "text/html,application/json,*/*"})
    try:
        with urllib.request.urlopen(req, timeout=20) as r:
            return r.status, r.read(400_000).decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        return e.code, ""
    except Exception:
        return 0, ""


def main():
    rows = list(csv.DictReader(open("independent/stations.tsv", encoding="utf-8"), delimiter="\t"))
    out = []
    for r in rows:
        site = (r.get("website") or "").strip()
        if not site or r.get("robots") not in ("allowed", "none"):
            continue
        base = f"https://{site.rstrip('/')}/"
        rp = urllib.robotparser.RobotFileParser()
        try:
            rp.parse(fetch(base + "robots.txt")[1].splitlines())
        except Exception:
            pass
        found = {"platform": set(), "feeds": set(), "tracks": set(), "shows": set()}
        status = ""
        for path in PATHS:
            url = base + path
            if path and not rp.can_fetch(UA, url):
                continue
            time.sleep(PAUSE)
            st, body = fetch(url)
            if path == "":
                status = str(st)
            if st != 200 or not body:
                continue
            low = body.lower()
            if path == "wp-json/":
                if '"namespaces"' in low:
                    found["feeds"].add("wp-json")
                continue
            if path in ("playlist", "tracklist", "recently-played"):
                found["tracks"].add(f"/{path} exists")
            if path in ("schedule", "shows", "programmes", "archive"):
                found["shows"].add(f"/{path} exists")
            for name, pat in PLATFORMS.items():
                if re.search(pat, low):
                    found["platform"].add(name)
            for m in re.finditer(r'<link[^>]+type="application/(?:rss|atom)\+xml"[^>]*href="([^"]+)"', body, re.I):
                found["feeds"].add(m.group(1)[:80])
            for m in re.finditer(r'<a[^>]+href="([^"#]+)"[^>]*>(.*?)</a>', body, re.I | re.S):
                text = re.sub(r"<[^>]+>", "", m.group(2)).strip()[:40]
                blob = (m.group(1) + " " + text).lower()
                if re.search(TRACK_WORDS, blob):
                    found["tracks"].add(f"{text or m.group(1)[:40]} -> {m.group(1)[:60]}")
                elif re.search(SHOW_WORDS, blob) and len(found["shows"]) < 6:
                    found["shows"].add(f"{text or m.group(1)[:40]} -> {m.group(1)[:60]}")
            if re.search(TRACK_WORDS, low) and not found["tracks"]:
                found["tracks"].add("mentioned in page text")
        out.append({"station": r["station"], "website": site, "status": status,
                    **{k: " | ".join(sorted(v))[:400] for k, v in found.items()},
                    "checked_on": date.today().isoformat()})
        print(r["station"], status, sorted(found["platform"]), "tracks:", len(found["tracks"]), flush=True)
    with open("independent/probe.tsv", "w", newline="", encoding="utf-8") as f:
        w = csv.DictWriter(f, fieldnames=["station", "website", "status", "platform", "feeds", "tracks", "shows", "checked_on"],
                           delimiter="\t", lineterminator="\n")
        w.writeheader()
        w.writerows(o for o in out)
    print(len(out), "stations probed")


if __name__ == "__main__":
    main()
