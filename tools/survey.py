#!/usr/bin/env python3
"""Survey the stations in independent/stations.tsv.

For each station with a website it records, in the same file:
  robots         allowed | blocked | none (no robots.txt) | unknown (could not be read)
  contact_email  email address(es) found on the home, /contact and /about pages
  checked_on     the date of this check

The rule (decided by the project owner, 13 Nov 2026):
  allowed  -> the station can be collected.
  blocked  -> the site is marked, an email address is found, and it is collected the same way as
              Jazz FM (see docs/JAZZ-FM.md), gently, until the station answers our email.
Nothing is collected here. This only reads robots.txt and up to three ordinary pages per site,
2 seconds apart, as RadioArchivesBot.

Usage: tools/survey.py [stations.tsv]
"""
import csv, re, sys, time, urllib.error, urllib.parse, urllib.request, urllib.robotparser
from datetime import date

UA = ("RadioArchivesBot/0.2 (+https://github.com/pink-bourbon-estate/Radio_Archives; "
      "unofficial non-commercial archive; a few requests per station)")
PAUSE = 2.0
EMAIL = re.compile(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}")
JUNK = re.compile(r"\.(png|jpe?g|gif|svg|webp|css|js)$|sentry|example\.|wixpress|godaddy|domain\.com", re.I)
NEW_COLUMNS = ["robots", "contact_email", "checked_on"]


def fetch(url):
    """(status, text). status 0 = could not connect."""
    req = urllib.request.Request(url, headers={"User-Agent": UA, "Accept": "text/html,text/plain"})
    try:
        with urllib.request.urlopen(req, timeout=20) as r:
            return r.status, r.read(500_000).decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        return e.code, ""
    except Exception:
        return 0, ""


def base_url(site):
    site = site.strip().rstrip("/")
    return site if "://" in site else "https://" + site


def robots_status(base):
    """allowed / blocked / none / unknown for the site root."""
    status, body = fetch(base + "/robots.txt")
    if status in (404, 410):
        return "none"
    if status != 200:
        return "unknown"
    rp = urllib.robotparser.RobotFileParser()
    rp.parse(body.splitlines())
    return "allowed" if rp.can_fetch("RadioArchivesBot", base + "/") else "blocked"


def emails_on(base):
    found = []
    for path in ("/", "/contact", "/about"):
        status, html = fetch(base + path)
        if status == 200:
            # JSON in pages escapes "<" and ">" as \u003c / \u003e; undo that before matching.
            text = re.sub(r"\\?u003[ce]", " ", urllib.parse.unquote(html), flags=re.I)
            for m in EMAIL.findall(text):
                m = m.lower().strip(".")
                if not JUNK.search(m) and m not in found:
                    found.append(m)
        time.sleep(PAUSE)
    return found[:3]


def survey(path):
    with open(path, newline="", encoding="utf-8") as f:
        rows = list(csv.DictReader(f, delimiter="\t"))
        cols = list(rows[0].keys()) if rows else []
    for c in NEW_COLUMNS:
        if c not in cols:
            cols.append(c)
    today = date.today().isoformat()
    for r in rows:
        site = (r.get("website") or "").strip()
        if not site or site.lower() == "none found":
            continue
        base = base_url(site)
        r["robots"] = robots_status(base)
        time.sleep(PAUSE)
        # Keep addresses already in the registry (found by hand) and add new ones.
        mails = [m.strip() for m in (r.get("contact_email") or "").split(";") if m.strip()]
        mails += [m for m in emails_on(base) if m not in mails]
        r["contact_email"] = "; ".join(mails[:4])
        r["checked_on"] = today
        print(f"{r['station']}: robots={r['robots']} emails={r['contact_email'] or '-'}")
    with open(path, "w", newline="", encoding="utf-8") as f:
        w = csv.DictWriter(f, fieldnames=cols, delimiter="\t", lineterminator="\n", extrasaction="ignore")
        w.writeheader()
        w.writerows(rows)


if __name__ == "__main__":
    survey(sys.argv[1] if len(sys.argv) > 1 else "independent/stations.tsv")
