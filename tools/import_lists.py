#!/usr/bin/env python3
"""Grow independent/stations.tsv from public lists of UK stations, then let tools/survey.py check
them. Run by the "Independent stations: import lists" workflow (my own sandbox cannot reach these
sites). Sources:

  1. Wikipedia, "List of community radio stations in the United Kingdom" (follows Ofcom's
     community licences), read through the MediaWiki API. Each station's website comes from the
     infobox of its own Wikipedia article, fetched 50 at a time.
  2. "The Golden Age of Community Radio" directory, UK page (radio.surfacepressure.net), mostly
     online-only stations. Each station page is read for its website.

Rules: robots.txt is checked for each site first; requests are 1.5 s apart; the bot identifies
itself. Stations already in the registry (same normalised name) are not duplicated; a missing
website on an existing row is filled in. The raw pages are saved in independent/sources/ so the
parsing can be checked and fixed.
"""
import csv, html, json, os, re, sys, time, urllib.parse, urllib.request, urllib.robotparser

UA = ("RadioArchivesBot/0.2 (+https://github.com/pink-bourbon-estate/Radio_Archives; "
      "unofficial non-commercial archive)")
PAUSE = 1.5
REG = "independent/stations.tsv"
SRC = "independent/sources"
WIKI_API = "https://en.wikipedia.org/w/api.php"
WIKI_LIST = "List of community radio stations in the United Kingdom"
GA_UK = "https://radio.surfacepressure.net/location/uk/"
SKIP_HOSTS = re.compile(r"(surfacepressure|facebook|instagram|twitter|x\.com|mixcloud|soundcloud|"
                        r"youtube|tiktok|linkedin|wikipedia|wikimedia|wordpress\.org|gravatar|"
                        r"google|apple\.com|spotify|patreon|threads\.net|bsky|w\.org)", re.I)
_robots = {}


def allowed(url):
    p = urllib.parse.urlsplit(url)
    root = f"{p.scheme}://{p.netloc}"
    if root not in _robots:
        rp = urllib.robotparser.RobotFileParser(root + "/robots.txt")
        try:
            rp.read()
        except Exception:
            rp = None
        _robots[root] = rp
    rp = _robots[root]
    return True if rp is None else rp.can_fetch("RadioArchivesBot", url)


def get(url):
    if not allowed(url):
        print(f"  robots.txt disallows {url}; skipped")
        return ""
    time.sleep(PAUSE)
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            return r.read().decode("utf-8", "replace")
    except Exception as e:
        print(f"  could not fetch {url}: {e}")
        return ""


def norm(name):
    n = re.sub(r"\(.*?\)", " ", name.lower())
    n = re.sub(r"[^a-z0-9 ]", " ", n.replace("&", " and "))
    words = [w for w in n.split() if w not in {"the", "radio", "fm", "community", "cic", "ltd"}]
    return "".join(words) or re.sub(r"[^a-z0-9]", "", name.lower())


def site(url):
    url = url.strip().strip("[]").split()[0] if url.strip() else ""
    url = re.sub(r"^https?://", "", url).rstrip("/")
    return re.sub(r"^www\.", "", url)


# ---- Wikipedia ------------------------------------------------------------------------------

def strip_wiki(t):
    t = re.sub(r"<ref[^>]*/>|<ref[^>]*>.*?</ref>", "", t, flags=re.S)
    t = re.sub(r"\{\{[^{}]*\}\}", "", t)
    t = re.sub(r"\[\[(?:[^|\]]*\|)?([^\]]*)\]\]", r"\1", t)
    t = re.sub(r"\[https?://\S+ ([^\]]*)\]", r"\1", t)
    t = re.sub(r"<[^>]+>|'''?", "", t)
    return html.unescape(t).strip()


def wiki_rows(text):
    """(name, article title or '', place, section) for each table row whose first cell links."""
    out, section, cells = [], "", []

    def flush():
        if len(cells) >= 2:
            m = re.search(r"\[\[([^|\]#]+)(?:\|([^\]]*))?\]\]", cells[0])
            name = strip_wiki(cells[0])
            if name and name.lower() not in {"station", "name"}:
                out.append((name, m.group(1).strip() if m else "", strip_wiki(cells[1]), section))
        cells.clear()

    for line in text.split("\n"):
        h = re.match(r"^==+\s*(.*?)\s*==+\s*$", line)
        if h:
            flush()
            section = strip_wiki(h.group(1))
        elif line.startswith(("|-", "|}", "{|")):
            flush()
        elif line.startswith("|") and not line.startswith("|+"):
            for c in line[1:].split("||"):
                if re.match(r'^\s*(style|align|rowspan|colspan|class|scope|bgcolor)=', c) and "|" in c:
                    c = c.split("|", 1)[1]
                cells.append(c.strip())
    flush()
    return out


def wiki_websites(titles):
    found = {}
    titles = [t for t in dict.fromkeys(titles) if t]
    for i in range(0, len(titles), 50):
        q = urllib.parse.urlencode({"action": "query", "prop": "revisions", "rvprop": "content",
                                    "rvslots": "main", "format": "json", "formatversion": "2",
                                    "redirects": "1", "titles": "|".join(titles[i:i + 50])})
        body = get(f"{WIKI_API}?{q}")
        if not body:
            continue
        data = json.loads(body)["query"]
        back = {r["to"]: r["from"] for r in data.get("redirects", [])}
        for p in data.get("pages", []):
            try:
                text = p["revisions"][0]["slots"]["main"]["content"]
            except (KeyError, IndexError):
                continue
            m = re.search(r"\|\s*website\s*=\s*([^\n]+)", text, flags=re.I)
            if not m:
                continue
            v = m.group(1)
            u = re.search(r"\{\{\s*url\s*\|\s*([^|}]+)", v, flags=re.I) or re.search(r"(https?://[^\s\]|}]+)", v) \
                or re.search(r"([a-z0-9-]+(?:\.[a-z0-9-]+)+\.[a-z]{2,}[^\s\]|}]*)", v, flags=re.I)
            if u:
                found[back.get(p["title"], p["title"])] = site(u.group(1))
    return found


def from_wikipedia():
    q = urllib.parse.urlencode({"action": "parse", "page": WIKI_LIST, "prop": "wikitext",
                                "format": "json", "formatversion": "2"})
    body = get(f"{WIKI_API}?{q}")
    if not body:
        return []
    text = json.loads(body)["parse"]["wikitext"]
    open(f"{SRC}/wikipedia-community-list.wikitext", "w").write(text)
    rows = wiki_rows(text)
    print(f"Wikipedia: {len(rows)} rows in the list")
    sites = wiki_websites([r[1] for r in rows])
    print(f"Wikipedia: websites found for {len(sites)} stations")
    return [{"station": n, "city": place or sec, "kind": "Ofcom community (Wikipedia list)",
             "website": sites.get(t, ""), "source": "wikipedia.org list of UK community radio"}
            for n, t, place, sec in rows]


# ---- Golden Age of Community Radio ----------------------------------------------------------

def from_golden_age():
    page = get(GA_UK)
    if not page:
        return []
    open(f"{SRC}/surfacepressure-uk.html", "w").write(page)
    links = dict.fromkeys(re.findall(r'href="(https://radio\.surfacepressure\.net/station/[^"/]+/)"', page))
    print(f"Golden Age: {len(links)} UK station pages")
    out = []
    for link in links:
        sp = get(link)
        t = re.search(r"<h1[^>]*>(.*?)</h1>", sp, flags=re.S) or re.search(r"<title>(.*?)(?:[|–-].*)?</title>", sp, flags=re.S)
        name = html.unescape(re.sub(r"<[^>]+>", "", t.group(1))).strip() if t else link.rstrip("/").rsplit("/", 1)[1]
        loc = re.findall(r'/location/([a-z0-9-]+)/', sp)
        loc = [l for l in loc if l not in {"uk", "england", "scotland", "wales", "europe", "planet-earth", "northern-ireland"}]
        ext = [u for u in re.findall(r'href="(https?://[^"]+)"', sp) if not SKIP_HOSTS.search(urllib.parse.urlsplit(u).netloc)]
        out.append({"station": name, "city": loc[0].replace("-", " ").title() if loc else "",
                    "kind": "online / community (Golden Age directory)",
                    "website": site(ext[0]) if ext else "", "source": "radio.surfacepressure.net"})
    return out


# ---- merge ----------------------------------------------------------------------------------

def merge(new):
    with open(REG, newline="", encoding="utf-8") as f:
        rows = list(csv.DictReader(f, delimiter="\t"))
        cols = list(rows[0].keys())
    by = {norm(r["station"]): r for r in rows}
    by_site = {r["website"].lower(): r for r in rows if r.get("website")}
    added = filled = 0
    for n in new:
        k = norm(n["station"])
        if not k:
            continue
        same = by.get(k) or (by_site.get(n["website"].lower()) if n.get("website") else None)
        if same:
            if not same.get("website") and n.get("website"):
                same["website"] = n["website"]
                by_site[n["website"].lower()] = same
                filled += 1
            continue
        row = {c: "" for c in cols}
        row.update(n)
        row.update({"status": "listed; not confirmed", "risk": "unknown", "track_data": "unchecked"})
        rows.append(row)
        by[k] = row
        if row.get("website"):
            by_site[row["website"].lower()] = row
        added += 1
    with open(REG, "w", newline="", encoding="utf-8") as f:
        w = csv.DictWriter(f, fieldnames=cols, delimiter="\t", lineterminator="\n", extrasaction="ignore")
        w.writeheader()
        w.writerows(rows)
    print(f"registry: +{added} stations, {filled} websites filled in, {len(rows)} in total")


if __name__ == "__main__":
    os.makedirs(SRC, exist_ok=True)
    merge(from_golden_age() + from_wikipedia())
