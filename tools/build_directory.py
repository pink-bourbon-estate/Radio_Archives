#!/usr/bin/env python3
"""Build the Independent Archive directory: the front page (stations by region), a profile page for every station,
and the linked data lists (stations, programmes, episodes) as downloads.

    python3 tools/build_directory.py OUT_DIR BASE      e.g. public/independent /Radio_Archives/independent/

The data is kept as linked lists: stations -> programmes -> episodes -> tracks. This builds the first three from what the
collectors have stored (independent/stations.tsv, independent/data/*). Tracks live in the track dashboard (/tracks/).
Contact emails are never written out. The pages load nothing from other sites and set no cookies."""
import csv, glob, html, json, os, re, shutil, sys, collections

REGION = {
    "London": "London", "Bristol": "South West", "Bath": "South West", "Taunton": "South West", "Totnes": "South West",
    "Falmouth": "South West", "Warminster": "South West", "Weston-super-Mare": "South West", "Cheshire": "North West",
    "Manchester": "North West", "Liverpool": "North West", "Wigan": "North West", "Barrow In Furness": "North West",
    "Staffordshire": "West Midlands", "Birmingham": "West Midlands", "Leicester": "East Midlands", "Northampton": "East Midlands",
    "Nottingham": "East Midlands", "West Norfolk": "East of England", "Brighton": "South East", "Margate": "South East",
    "Kent": "South East", "Leeds": "Yorkshire and the Humber", "Sheffield": "Yorkshire and the Humber",
    "Bradford": "Yorkshire and the Humber", "Doncaster": "Yorkshire and the Humber", "Edinburgh": "Scotland",
    "Glasgow": "Scotland", "Dundee": "Scotland", "Penicuik": "Scotland", "Rutherglen": "Scotland", "Peterhead": "Scotland",
    "Stornoway": "Scotland", "Avondale": "Scotland", "Cardiff": "Wales", "Swansea": "Wales", "Belfast": "Northern Ireland"}
RISK = {"closed": "Closed", "paused": "Paused", "high": "At risk", "watch": "Small, self-funded",
        "unknown": "Not confirmed", "low": "Stable"}
e = html.escape


EXTRA_CSS = """
.slicer { display: flex; flex-wrap: wrap; gap: 10px 16px; align-items: flex-start; margin: 16px 0; }
.slicer details { border: 1px solid var(--line); padding: 8px 12px; min-width: 260px; }
.slicer summary { cursor: pointer; }
.slicer select, .slicer button, .tools select { font: inherit; font-size: 14px; padding: 6px 10px; border: 1px solid var(--ink); background: transparent; color: var(--ink); border-radius: 0; cursor: pointer; }
.plist { max-height: 260px; overflow: auto; margin-top: 8px; }
.pick { display: block; font-size: 14px; padding: 2px 0; }
.grid2 { display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 0 40px; }
.hb { display: grid; grid-template-columns: 9rem 1fr 3.5rem; align-items: center; gap: 10px; font-size: 13px; padding: 3px 0; }
.hb i { display: block; height: 10px; background: var(--ink); }
.hb em { font-style: normal; text-align: right; color: var(--mute); }
.heatwrap { overflow-x: auto; }
.hm { border-collapse: collapse; font-size: 11px; }
.hm th { font-weight: 400; text-transform: none; letter-spacing: 0; padding: 2px 6px 2px 0; border: 0; white-space: nowrap; }
.hm td { width: 14px; height: 18px; padding: 0; border: 1px solid var(--bg); }
.facts { display: grid; grid-template-columns: 9rem 1fr; gap: 6px 16px; font-size: 14px; }
.facts dt { color: var(--mute); } .facts dd { margin: 0; }
.tools label { font-size: 14px; display: inline-flex; gap: 8px; align-items: center; }
"""


ALIAS = {"hiphop": "hip hop", "rnb": "r&b", "randb": "r&b", "rn b": "r&b", "dnb": "drum & bass", "drum and bass": "drum & bass",
         "drumandbass": "drum & bass", "drum n bass": "drum & bass", "electronica": "electronica"}


def clean_tag(t, stationkeys):
    """Genre-like tags only: lower-cased, merged spellings, no station names, no numbers."""
    t = html.unescape(t).strip().lower()
    t = re.sub(r"\s+", " ", t)
    flat = re.sub(r"[^a-z0-9]", "", t)
    if not t or t.isdigit() or len(flat) < 2 or flat in stationkeys:
        return ""
    if any(w in flat for w in ("radio", "listenagain", "fm", "podcast", "weeklyshow")) and flat not in ("fmradio",):
        return ""
    t = re.sub(r"[''`]", "", t).replace(" n ", " & ") if "n" in t and re.search(r"\b(r|d)\s?n\s?[bs]\b|rn?n?b", t) else t
    return ALIAS.get(t, ALIAS.get(re.sub(r"[^a-z0-9& ]", "", t), t))


def slug(s):
    return re.sub(r"[^a-z0-9]+", "-", s.lower()).strip("-")


def region_of(city):
    base = re.split(r"\s*[(,]", city)[0].strip()
    base = {"North Bristol": "Bristol", "South Bristol": "Bristol", "Glasgow (Govan)": "Glasgow"}.get(city, base)
    return REGION.get(base, "Not yet placed" if not base else REGION.get(base.split()[-1], "Not yet placed"))


def tsv(path):
    return list(csv.DictReader(open(path, encoding="utf-8"), delimiter="\t")) if os.path.exists(path) else []


def jsonl(path):
    return [json.loads(l) for l in open(path, encoding="utf-8") if l.strip()] if os.path.exists(path) else []


def page(base, title, desc, body, active=""):
    nav = "".join(f'<a href="{base}{h}"{" aria-current=page" if k == active else ""}>{t}</a>' for k, h, t in
                  [("dir", "", "Stations"), ("tracks", "tracks/", "Tracks"), ("about", "about/", "About")])
    return f"""<!doctype html>
<html lang="en-GB"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>{e(title)}</title><meta name="description" content="{e(desc)}"><meta name="robots" content="noindex">
<link rel="icon" href="{base}assets/favicon.svg" type="image/svg+xml"><script src="{base}assets/theme.js"></script>
<link rel="stylesheet" href="{base}assets/style.css"></head><body>
<a class="skip" href="#main">Skip to the content</a>
<header class="top"><a class="mark" href="{base}" aria-label="Independent Archive, home">Independent <span>Archive</span></a>
<nav aria-label="Main">{nav}</nav></header>
<main id="main" tabindex="-1">{body}</main>
<footer class="foot"><p>An unofficial, non-commercial archive. Not made, endorsed or checked by any station. No cookies, no tracking, no adverts, no money.
Every station links to its own website. Part of <a href="https://pink-bourbon-estate.github.io/">Pink Bourbon Estate</a>.</p></footer></body></html>"""


def main(out, base):
    stations = tsv("independent/stations.tsv")
    probe = {r["station"]: r for r in tsv("independent/probe.tsv")}
    accounts = {r["station"]: r["mixcloud_user"] for r in tsv("independent/mixcloud_accounts.tsv")}
    noods = jsonl("independent/data/noods/shows.jsonl")
    mix = {}
    for st, u in accounts.items():
        mix[st] = jsonl(f"independent/data/mixcloud/{u}.jsonl")

    episodes = []  # station, programme, date, title, tags, url, tracks
    for s in noods:
        episodes.append(("Noods Radio", s["title"], s["date"], s["title"], "; ".join(s["genres"]),
                         f"https://noodsradio.com/shows/{s['slug']}", len(s["tracks"]), int(s["start"][:2]) if s.get("start") else -1))
    for st, rows in mix.items():
        for r in rows:
            episodes.append((st, "", r["date"], r["title"], "; ".join(r["tags"]), r["url"], len(r["sections"] or []), -1))

    by_station = collections.defaultdict(list)
    for ep in episodes:
        by_station[ep[0]].append(ep)

    shutil.rmtree(os.path.join(out, "station"), ignore_errors=True)
    os.makedirs(os.path.join(out, "data"), exist_ok=True)
    recs = []
    for s in stations:
        name = s["station"]
        eps = sorted(by_station.get(name, []), key=lambda x: x[2], reverse=True)
        dates = [x[2] for x in eps if x[2]]
        tracks = sum(x[6] for x in eps)
        pr = probe.get(name, {})
        r = {"name": name, "slug": slug(name), "city": s["city"], "region": region_of(s["city"]), "kind": s["kind"],
             "music": s["music"], "status": s["status"], "risk": s["risk"], "website": s["website"].split("?")[0],
             "notes": s["notes"], "episodes": len(eps), "first": min(dates) if dates else "", "last": max(dates) if dates else "",
             "tracks": tracks, "mixcloud": accounts.get(name, ""), "platforms": pr.get("platform", ""), "eps": eps}
        recs.append(r)
    recs.sort(key=lambda r: (r["region"], r["city"].lower(), r["name"].lower()))

    # data for the dashboard: compact arrays, no emails
    import datetime
    idx = {r["name"]: i for i, r in enumerate(recs)}
    stationkeys = {re.sub(r"[^a-z0-9]", "", r["name"].lower()) for r in recs}
    tagi, taglist = {}, []
    dep = []
    for r in recs:
        for x in r["eps"]:
            if not x[2]:
                continue
            ids = []
            for t in x[4].split(";"):
                k = clean_tag(t, stationkeys)
                if k:
                    if k not in tagi:
                        tagi[k] = len(taglist)
                        taglist.append((k[0].upper() + k[1:]).replace("R&b", "R&B"))
                    ids.append(tagi[k])
            day = (datetime.date.fromisoformat(x[2]) - datetime.date(1970, 1, 1)).days
            dep.append([idx[r["name"]], day, sorted(set(ids)), x[7], x[6]])
    json.dump({"st": [[r["slug"], r["name"], r["region"], r["city"]] for r in recs], "tags": taglist, "ep": dep},
              open(os.path.join(out, "data", "dash.json"), "w", encoding="utf-8"), separators=(",", ":"), ensure_ascii=False)

    # downloads: the linked lists (no emails anywhere)
    with open(os.path.join(out, "data", "stations.tsv"), "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f, delimiter="\t", lineterminator="\n")
        w.writerow(["station_id", "station", "town", "region", "type", "music", "status", "risk", "website", "mixcloud", "episodes_held", "tracks_held", "notes"])
        for r in recs:
            w.writerow([r["slug"], r["name"], r["city"], r["region"], r["kind"], r["music"], r["status"], r["risk"], r["website"], r["mixcloud"], r["episodes"], r["tracks"], r["notes"]])
    progs = collections.Counter((x[0], x[1]) for x in episodes if x[1])
    with open(os.path.join(out, "data", "programmes.tsv"), "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f, delimiter="\t", lineterminator="\n")
        w.writerow(["station_id", "programme", "episodes_held"])
        for (st, p), n in sorted(progs.items()):
            w.writerow([slug(st), p, n])
    os.makedirs(os.path.join(out, "data", "episodes"), exist_ok=True)
    for r in recs:  # one file per station, so no download is large
        if not r["eps"]:
            continue
        with open(os.path.join(out, "data", "episodes", f"{r['slug']}.tsv"), "w", newline="", encoding="utf-8") as f:
            w = csv.writer(f, delimiter="\t", lineterminator="\n")
            w.writerow(["station_id", "programme", "date", "title", "tags", "link", "tracks_listed"])
            for x in sorted(r["eps"], key=lambda x: x[2]):
                w.writerow([r["slug"], x[1], x[2], x[3], x[4], x[5], x[6]])

    # station pages
    for r in recs:
        site = f'<a href="https://{e(r["website"])}" rel="noopener">{e(r["website"])}</a>' if r["website"] else "No website found"
        mc = f'<a href="https://www.mixcloud.com/{e(r["mixcloud"])}/" rel="noopener">Mixcloud</a>' if r["mixcloud"] else ""
        facts = [("Where", e(r["city"] or "Not yet placed") + f" · {e(r['region'])}"), ("Type", e(r["kind"])), ("Music", e(r["music"])),
                 ("Status", e(r["status"]) + (f" — {e(r['notes'])}" if r["notes"] else "")),
                 ("Website", site), ("Hear the shows", " · ".join(x for x in [site, mc] if x)),
                 ("Platforms seen", e(r["platforms"]) or "Not checked")]
        dl = "".join(f"<dt>{k}</dt><dd>{v}</dd>" for k, v in facts if v)
        stats = (f'<div class="stats"><div class="stat"><b>{r["episodes"]:,}</b><span>shows held</span></div>'
                 f'<div class="stat"><b>{r["tracks"]:,}</b><span>tracks listed</span></div>'
                 f'<div class="stat"><b>{e(r["first"][:4]) or "–"}–{e(r["last"][:4]) or "–"}</b><span>years covered</span></div></div>')
        tags = collections.Counter(t.strip() for x in r["eps"] for t in x[4].split(";") if t.strip())
        tagp = ("<h2>Genres its shows are tagged with</h2><p>" + " · ".join(f"{e(t)} ({n})" for t, n in tags.most_common(15)) + "</p>") if tags else ""
        recent = "".join(f'<tr><td>{e(x[2])}</td><td><a href="{e(x[5])}" rel="noopener">{e(x[3])}</a></td><td>{e(x[4])}</td><td>{x[6] or ""}</td></tr>' for x in r["eps"][:25])
        rec = (f'<h2>Latest shows held</h2><p class="meta"><a href="{base}data/episodes/{r['slug']}.tsv">Download all {r['episodes']:,}</a></p><div class="wrap"><table><thead><tr><th>Date</th><th>Show (links to the station)</th><th>Tags</th><th>Tracks</th></tr></thead><tbody>{recent}</tbody></table></div>') if recent else "<h2>Shows</h2><p>No shows collected yet for this station.</p>"
        trk = ""
        if r["episodes"]:
            trk = f'<p><a href="{base}?station={r["slug"]}">See it in the dashboard →</a>'
            if r["tracks"]:
                trk += f' · <a href="{base}tracks/">Track statistics</a>'
            trk += "</p>"
        if r["episodes"] and not r["tracks"]:
            trk += "<p>This station does not publish tracklists, so no tracks are listed.</p>"
        body = (f'<p class="meta"><a href="{base}">← All stations</a></p><h1>{e(r["name"])}</h1><p class="lede">{e(r["city"] or "")}'
                f'{" · " + e(r["region"]) if r["region"] else ""}</p>{stats}<dl class="facts">{dl}</dl>{trk}{tagp}{rec}'
                f'<p class="meta">Is this your station? To correct or remove this page, open an issue on the '
                f'<a href="https://github.com/pink-bourbon-estate/Radio_Archives/issues" rel="noopener">source code page</a>. It will be done promptly.</p>')
        d = os.path.join(out, "station", r["slug"])
        os.makedirs(d, exist_ok=True)
        open(os.path.join(d, "index.html"), "w", encoding="utf-8").write(page(base, f'{r["name"]} · Independent Archive', f'{r["name"]}, {r["city"]}: shows and details.', body))

    # front page
    reg = collections.defaultdict(list)
    for r in recs:
        reg[r["region"]].append(r)
    cards = "".join(f'<div class="card"><h3>{e(k)}</h3><p>{len(v)} station{"s" if len(v) != 1 else ""}; {sum(1 for x in v if x["episodes"])} with shows held</p></div>'
                    for k, v in sorted(reg.items(), key=lambda kv: -len(kv[1])))
    rows = "".join(
        f'<tr data-region="{e(r["region"])}" data-risk="{e(r["risk"])}"><td><a href="{base}station/{r["slug"]}/">{e(r["name"])}</a></td><td>{e(r["city"])}</td><td>{e(r["region"])}</td>'
        f'<td>{e(r["music"])}</td><td>{e(RISK.get(r["risk"], r["risk"]))}</td><td>{r["episodes"]:,}</td><td>{r["tracks"]:,}</td></tr>' for r in recs)
    ropts = "".join(f'<option>{e(k)}</option>' for k in sorted(reg))
    body = f"""<h1>Independent Archive</h1>
<p class="lede">What UK independent and community radio is broadcasting: which stations, where, which shows and genres, and when. Pick one or more stations to filter everything. Tracks are listed only where a station publishes them.</p>
<noscript><p>The dashboard needs JavaScript. The <a href="{base}data/stations.tsv">station list</a> can be downloaded without it.</p></noscript>
<div id="dash">
<div class="slicer"><details><summary>Stations: <span id="sel">All stations</span></summary>
<input id="sq" type="search" placeholder="Find a station" aria-label="Find a station"><div id="slist" class="plist"></div></details>
<select id="reg" aria-label="Region"><option value="">All regions</option></select><button id="clear" type="button">Clear</button></div>
<div class="stats" id="stats"></div>
<h2>Shows over time</h2><div id="tc"></div>
<div class="grid2"><div><h2>Genres</h2><div id="genres"></div></div><div><h2>Day of the week</h2><div id="dow"></div></div></div>
<h2>Start time</h2><p class="meta" id="hournote"></p><div class="grid2"><div id="hours"></div><div><div id="heat" class="heatwrap"></div></div></div>
<h2>Where</h2><div id="regions"></div>
<h2>Stations</h2><div class="wrap"><table><thead><tr><th>Station</th><th>Town / city</th><th>Region</th><th>Shows</th><th>Tracks</th></tr></thead><tbody id="tbody"></tbody></table></div>
</div>
<p class="meta">Two views: this dashboard (with a page for every station) and <a href="{base}tracks/">Tracks</a> (statistics for stations that publish tracklists, so far Noods Radio). Why this exists: <a href="https://pink-bourbon-estate.github.io/articles/keeping-a-record-of-small-radio-before-it-disappears/">the blog post</a>. Show dates for stations on Mixcloud are upload dates; Noods dates and start times are broadcast. Stations are listed from public sources: "not yet placed" or "none found" means we have not found one, not that none exists.</p>
<p class="meta">Downloads: <a href="{base}data/stations.tsv">stations</a> · <a href="{base}data/programmes.tsv">programmes</a> · episodes (one file per station, linked from its page). Tab-separated; they link by <code>station_id</code>.</p>
<script src="{base}assets/timeseries.js"></script><script src="{base}assets/dash.js" data-base="{base}"></script>"""
    os.makedirs(out, exist_ok=True)
    open(os.path.join(out, "index.html"), "w", encoding="utf-8").write(page(base, "Independent Archive", "A directory of UK independent and community radio stations.", body, "dir"))
    about = open("independent/site/about.html", encoding="utf-8").read()
    os.makedirs(os.path.join(out, "about"), exist_ok=True)
    open(os.path.join(out, "about", "index.html"), "w", encoding="utf-8").write(page(base, "About · Independent Archive", "About the Independent Archive.", about, "about"))
    os.makedirs(os.path.join(out, "assets"), exist_ok=True)
    for f in ("style.css", "theme.js", "favicon.svg"):
        shutil.copy(f"stations/bbc-radio-6-music/site/assets/{f}", os.path.join(out, "assets", f))
    for f in ("timeseries.js", "dash.js"):
        shutil.copy(f"independent/site/{f}", os.path.join(out, "assets", f))
    with open(os.path.join(out, "assets", "style.css"), "a", encoding="utf-8") as f:
        f.write(EXTRA_CSS)
    print(f"{len(recs)} station pages, {len(episodes)} shows, {sum(r['tracks'] for r in recs)} tracks")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
