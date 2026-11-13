#!/usr/bin/env python3
"""Grow independent/stations.tsv from public lists of UK stations, then let tools/survey.py check
them. Run by the "Independent stations: import lists" workflow (my own sandbox cannot reach these
sites). Sources:

  1. Wikipedia, "List of community radio stations in the United Kingdom" (follows Ofcom's
     community licences), read from the ordinary article page (Wikipedia's robots.txt turns
     bots away from the API and the ?action=raw text, but not from article pages). Each
     station's website comes from the infobox of its own article, one request per article.
  2. "The Golden Age of Community Radio" directory, UK page (radio.surfacepressure.net), mostly
     online-only stations. Each station page is read for its website.

Rules: robots.txt is checked for each site first; requests are 1.5 s apart; the bot identifies
itself. Stations already in the registry (same normalised name) are not duplicated; a missing
website on an existing row is filled in. The raw pages are saved in independent/sources/ so the
parsing can be checked and fixed.
"""
from html.parser import HTMLParser
import csv, html, os, re, sys, time, urllib.parse, urllib.request, urllib.robotparser

UA = ("RadioArchivesBot/0.2 (+https://github.com/pink-bourbon-estate/Radio_Archives; "
      "unofficial non-commercial archive)")
PAUSE = 1.5
REG = "independent/stations.tsv"
SRC = "independent/sources"
WIKI = "https://en.wikipedia.org/wiki/"
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

class WikiTables(HTMLParser):
    """Rows of every wikitable in a rendered Wikipedia page: (cells as text, first link in the
    first cell, the latest h2/h3 heading)."""

    def __init__(self):
        super().__init__()
        self.rows, self.heading = [], ""
        self.in_table = self.in_cell = self.in_head = 0
        self.row = self.link = None
        self.buf = ""

    def handle_starttag(self, tag, a):
        a = dict(a)
        if tag in ("h2", "h3"):
            self.in_head, self.buf = 1, ""
        elif tag == "table" and "wikitable" in (a.get("class") or ""):
            self.in_table += 1
        elif self.in_table and tag == "tr":
            self.row, self.link = [], None
        elif self.in_table and tag in ("td", "th") and self.row is not None:
            self.in_cell, self.buf = 1, ""
        elif self.in_cell and tag == "a" and not self.row and self.link is None:
            h = a.get("href", "")
            if h.startswith("/wiki/") and ":" not in h:
                self.link = urllib.parse.unquote(h[6:]).replace("_", " ")
        elif self.in_cell and tag in ("sup", "style"):
            self.in_cell = 2  # skip reference marks

    def handle_endtag(self, tag):
        if tag in ("h2", "h3") and self.in_head:
            self.heading, self.in_head = self.buf.replace("[edit]", "").strip(), 0
        elif tag in ("sup", "style") and self.in_cell == 2:
            self.in_cell = 1
        elif tag in ("td", "th") and self.in_cell:
            self.row.append(" ".join(self.buf.split()))
            self.in_cell = 0
        elif tag == "tr" and self.row is not None:
            if len(self.row) >= 2:
                self.rows.append((self.row, self.link, self.heading))
            self.row = None
        elif tag == "table" and self.in_table:
            self.in_table -= 1

    def handle_data(self, d):
        if self.in_head or self.in_cell == 1:
            self.buf += d


def website_in(page):
    """The website in a rendered article's infobox."""
    m = re.search(r'<th[^>]*>\s*Website\s*</th>\s*<td[^>]*>(.*?)</td>', page, flags=re.S | re.I)
    u = re.search(r'href="(https?://[^"]+)"', m.group(1)) if m else None
    return site(u.group(1)) if u else ""


def from_wikipedia():
    page = get(WIKI + urllib.parse.quote(WIKI_LIST.replace(" ", "_")))
    if not page:
        return []
    open(f"{SRC}/wikipedia-community-list.html", "w").write(page)
    p = WikiTables()
    p.feed(page)
    rows = [(c[0], link, c[1], h) for c, link, h in p.rows
            if c[0] and c[0].lower() not in {"station", "name", "station name"}]
    print(f"Wikipedia: {len(rows)} rows in the list")
    sites = {}
    for t in dict.fromkeys(r[1] for r in rows if r[1]):
        w = website_in(get(WIKI + urllib.parse.quote(t.replace(" ", "_"))))
        if w:
            sites[t] = w
    print(f"Wikipedia: websites found for {len(sites)} stations")
    return [{"station": re.sub(r"\[\d+\]", "", n).strip(), "city": place or sec,
             "kind": "Ofcom community (Wikipedia list)", "website": sites.get(t, ""),
             "source": "wikipedia.org list of UK community radio"}
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
