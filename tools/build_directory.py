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
                         f"https://noodsradio.com/shows/{s['slug']}", len(s["tracks"])))
    for st, rows in mix.items():
        for r in rows:
            episodes.append((st, "", r["date"], r["title"], "; ".join(r["tags"]), r["url"], len(r["sections"] or [])))

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
        trk = f'<p><a href="{base}tracks/">See its track statistics →</a></p>' if r["tracks"] else ("<p>This station does not publish tracklists, so no tracks are listed.</p>" if r["episodes"] else "")
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
<p class="lede">A directory of UK independent and community radio: who is on air, where, and what they broadcast. Tracks are listed only where a station publishes them.</p>
<div class="stats"><div class="stat"><b>{len(recs)}</b><span>stations</span></div><div class="stat"><b>{len(reg)}</b><span>regions and nations</span></div>
<div class="stat"><b>{sum(1 for r in recs if r['episodes'])}</b><span>with shows held</span></div><div class="stat"><b>{sum(r['episodes'] for r in recs):,}</b><span>shows</span></div>
<div class="stat"><b>{sum(r['tracks'] for r in recs):,}</b><span>tracks</span></div></div>
<p class="meta">Two views: <b>Stations</b> (this page, with a page for each station) and <a href="{base}tracks/">Tracks</a> (statistics for the stations that publish tracklists, so far Noods Radio). Why this exists: <a href="https://pink-bourbon-estate.github.io/articles/keeping-a-record-of-small-radio-before-it-disappears/">the blog post</a>. Stations are listed from public sources and checked over time; "not yet placed" or "none found" means we have not found one, not that none exists.</p>
<h2>By region</h2><div class="grid">{cards}</div>
<h2>All stations</h2>
<div class="tools"><input id="q" type="search" placeholder="Search station, town or music" aria-label="Search stations">
<select id="reg" aria-label="Region"><option value="">All regions</option>{ropts}</select></div>
<div class="wrap"><table><thead><tr><th>Station</th><th>Town / city</th><th>Region</th><th>Music</th><th>Risk</th><th>Shows held</th><th>Tracks</th></tr></thead><tbody id="t">{rows}</tbody></table></div>
<p class="meta">Downloads: <a href="{base}data/stations.tsv">stations</a> · <a href="{base}data/programmes.tsv">programmes</a> · episodes (one file per station, linked from its page). Tab-separated; they link to each other by <code>station_id</code>.</p>
<script>const R=[...document.querySelectorAll('#t tr')],q=document.getElementById('q'),g=document.getElementById('reg');
function go(){{const s=q.value.toLowerCase();R.forEach(r=>{{r.hidden=!((!g.value||r.dataset.region===g.value)&&r.textContent.toLowerCase().includes(s));}});}}
q.addEventListener('input',go);g.addEventListener('change',go);</script>"""
    os.makedirs(out, exist_ok=True)
    open(os.path.join(out, "index.html"), "w", encoding="utf-8").write(page(base, "Independent Archive", "A directory of UK independent and community radio stations.", body, "dir"))
    about = open("independent/site/about.html", encoding="utf-8").read()
    os.makedirs(os.path.join(out, "about"), exist_ok=True)
    open(os.path.join(out, "about", "index.html"), "w", encoding="utf-8").write(page(base, "About · Independent Archive", "About the Independent Archive.", about, "about"))
    for f in ("style.css", "theme.js", "favicon.svg"):
        os.makedirs(os.path.join(out, "assets"), exist_ok=True)
        shutil.copy(f"stations/bbc-radio-6-music/site/assets/{f}", os.path.join(out, "assets", f))
    print(f"{len(recs)} station pages, {len(episodes)} shows, {sum(r['tracks'] for r in recs)} tracks")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
