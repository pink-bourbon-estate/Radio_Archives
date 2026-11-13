#!/usr/bin/env python3
"""Build the Independent Archive page: the article plus the station registry as a table.
Usage: tools/build_independent.py OUT_DIR   (writes OUT_DIR/index.html and OUT_DIR/stations.tsv)
The page is one self-contained file: no scripts or fonts from other sites, no cookies."""
import csv, html, os, sys

RISK_ORDER = ["closed", "paused", "high", "watch", "unknown", "low"]
RISK_LABEL = {"closed": "Closed", "paused": "Paused", "high": "At risk", "watch": "Small, self-funded",
              "unknown": "Not confirmed", "low": "Stable"}


def main(out):
    os.makedirs(out, exist_ok=True)
    rows = list(csv.DictReader(open("independent/stations.tsv", encoding="utf-8"), delimiter="\t"))
    rows.sort(key=lambda r: (RISK_ORDER.index(r["risk"]) if r["risk"] in RISK_ORDER else 9,
                             r["city"].lower(), r["station"].lower()))
    counts = {k: sum(1 for r in rows if r["risk"] == k) for k in RISK_ORDER}
    e = html.escape
    trs = []
    for r in rows:
        name = e(r["station"])
        if r.get("website"):
            name = f'<a href="https://{e(r["website"])}" rel="noopener">{name}</a>'
        trs.append(f'<tr data-risk="{e(r["risk"])}"><td>{name}</td><td>{e(r["city"])}</td>'
                   f'<td>{e(r["music"])}</td><td><span class="risk {e(r["risk"])}">{e(RISK_LABEL.get(r["risk"], r["risk"]))}</span></td>'
                   f'<td>{e(r["status"])}{" — " + e(r["notes"]) if r["notes"] else ""}</td></tr>')
    buttons = "".join(f'<button data-f="{k}">{RISK_LABEL[k]} ({counts[k]})</button>' for k in RISK_ORDER if counts[k])
    article = open("independent/site/article.html", encoding="utf-8").read()
    page = f"""<!doctype html>
<html lang="en-GB"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>The Independent Archive</title>
<meta name="description" content="A record of what small UK independent and community radio stations play, kept before it disappears.">
<meta name="robots" content="noindex">
<style>
:root {{ color-scheme: light dark; }}
body {{ font: 16px/1.6 system-ui, sans-serif; max-width: 52rem; margin: 2.5rem auto; padding: 0 1.25rem; }}
h1 {{ font-weight: 500; font-size: 2.3rem; margin: 0 0 .25rem; }}
h2 {{ font-weight: 500; margin-top: 2.2rem; }}
.lede {{ opacity: .75; font-size: 1.1rem; }}
nav a {{ font-size: .9rem; }}
.sources {{ font-size: .9rem; }}
input {{ font: inherit; padding: .4rem .6rem; width: 100%; box-sizing: border-box; margin: .5rem 0; }}
button {{ font: inherit; font-size: .85rem; margin: 0 .3rem .4rem 0; padding: .25rem .6rem; border: 1px solid #8886; background: none; border-radius: 1rem; cursor: pointer; }}
button.on {{ background: #8883; }}
.wrap {{ overflow-x: auto; }}
table {{ border-collapse: collapse; width: 100%; font-size: .9rem; }}
th, td {{ text-align: left; padding: .45rem .5rem; border-top: 1px solid #8884; vertical-align: top; }}
td:last-child {{ min-width: 15rem; }}
td:first-child {{ min-width: 8rem; }}
.risk {{ white-space: nowrap; font-size: .8rem; padding: .05rem .45rem; border-radius: .6rem; border: 1px solid #8886; }}
.risk.closed, .risk.high {{ border-color: #c44; }}
.risk.paused {{ border-color: #c84; }}
footer {{ margin-top: 3rem; opacity: .7; font-size: .85rem; }}
</style></head><body>
<nav><a href="../">← Independent Archive</a></nav>
{article}
<h2 id="stations">The stations ({len(rows)})</h2>
<p>Every station found so far. A station marked "Not confirmed" came from a public list and has not been checked yet. <a href="stations.tsv">Download the list</a>.</p>
<div>{buttons}</div>
<input id="q" type="search" placeholder="Search by station, town or music" aria-label="Search stations">
<div class="wrap"><table><thead><tr><th>Station</th><th>Town / city</th><th>Music</th><th>Risk</th><th>Status</th></tr></thead>
<tbody id="t">{''.join(trs)}</tbody></table></div>
<footer>Unofficial and non-commercial. Not made, endorsed or checked by any station. Part of <a href="https://pink-bourbon-estate.github.io/">Pink Bourbon Estate</a>.</footer>
<script>
const rows=[...document.querySelectorAll('#t tr')],q=document.getElementById('q');let f='';
function go(){{const s=q.value.toLowerCase();rows.forEach(r=>{{r.hidden=!((!f||r.dataset.risk===f)&&r.textContent.toLowerCase().includes(s));}});}}
q.addEventListener('input',go);
document.querySelectorAll('button[data-f]').forEach(b=>b.addEventListener('click',()=>{{f=f===b.dataset.f?'':b.dataset.f;document.querySelectorAll('button[data-f]').forEach(x=>x.classList.toggle('on',x.dataset.f===f));go();}}));
</script>
</body></html>
"""
    open(os.path.join(out, "index.html"), "w", encoding="utf-8").write(page)
    # The public copy leaves out contact emails: they are for the project's own outreach only.
    cols = [c for c in rows[0].keys() if c != "contact_email"]
    with open(os.path.join(out, "stations.tsv"), "w", newline="", encoding="utf-8") as f:
        w = csv.DictWriter(f, fieldnames=cols, delimiter="\t", lineterminator="\n", extrasaction="ignore")
        w.writeheader()
        w.writerows(rows)
    print(f"wrote {out}/index.html with {len(rows)} stations")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "public/independent")
