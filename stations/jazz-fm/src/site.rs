//! A small static page for the station: totals, plays per day, top artists and tracks, the
//! latest plays, and a CSV of everything. Plain HTML, no JavaScript, no external requests.

use catalogue::Catalogue;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io;
use std::path::Path;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn urlenc(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

fn csv(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

fn top(counts: HashMap<String, usize>, n: usize) -> Vec<(String, usize)> {
    let mut v: Vec<_> = counts.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v.truncate(n);
    v
}

pub fn build(plays: &[(String, String)], cat: &Catalogue, out: &Path, noindex: bool) -> io::Result<()> {
    fs::create_dir_all(out)?;
    let name_of = |tid: &str| -> (String, String) {
        match cat.tracks.get(tid) {
            Some(t) => (cat.artist_name(&t.artist_id).to_string(), t.title.clone()),
            None => ("?".into(), tid.to_string()),
        }
    };

    // CSV of everything
    let mut c = String::from("played_at_utc,artist,title\n");
    for (at, tid) in plays {
        let (a, t) = name_of(tid);
        c.push_str(&format!("{at},{},{}\n", csv(&a), csv(&t)));
    }
    fs::write(out.join("plays.csv"), c)?;

    let mut per_day: BTreeMap<&str, usize> = BTreeMap::new();
    let mut artists: HashMap<String, usize> = HashMap::new();
    let mut tracks: HashMap<String, usize> = HashMap::new();
    for (at, tid) in plays {
        *per_day.entry(&at[..10]).or_default() += 1;
        *tracks.entry(tid.clone()).or_default() += 1;
        if let Some(t) = cat.tracks.get(tid) {
            *artists.entry(t.artist_id.clone()).or_default() += 1;
        }
    }

    let mut h = String::new();
    h.push_str("<!doctype html>\n<html lang=\"en-GB\"><head><meta charset=\"utf-8\">\n");
    h.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    h.push_str("<title>Jazz FM archive</title>\n");
    if noindex {
        h.push_str("<meta name=\"robots\" content=\"noindex\">\n");
    }
    h.push_str("<style>:root{color-scheme:light dark}body{font:16px/1.5 system-ui,sans-serif;max-width:60rem;margin:2rem auto;padding:0 1.25rem}h1{font-weight:500}h2{margin-top:2.5rem;font-weight:500}table{border-collapse:collapse;width:100%}td,th{text-align:left;padding:.3rem .6rem;border-top:1px solid #8884}td.n{text-align:right;font-variant-numeric:tabular-nums}.cols{display:grid;grid-template-columns:repeat(auto-fit,minmax(24rem,1fr));gap:2rem}.meta,footer{opacity:.7;font-size:.9rem}</style>\n");
    h.push_str("</head><body>\n<p><a href=\"../\">&larr; All stations</a></p>\n<h1>Jazz FM archive</h1>\n");

    if plays.is_empty() {
        h.push_str("<p>No plays collected yet. The first run fills this page.</p>\n");
    } else {
        let first = &plays[0].0;
        let last = &plays[plays.len() - 1].0;
        h.push_str(&format!(
            "<p class=\"meta\">{} plays of {} tracks by {} artists, collected from {} to {} (UTC). \
             The station only publishes about a week of history, so this archive starts on the first day it was collected. \
             <a href=\"plays.csv\">Download everything as CSV</a>.</p>\n",
            plays.len(), tracks.len(), artists.len(), &first[..10], &last[..10]
        ));

        h.push_str("<div class=\"cols\"><div><h2>Top artists</h2><table>\n");
        for (id, n) in top(artists, 25) {
            h.push_str(&format!("<tr><td>{}</td><td class=\"n\">{n}</td></tr>\n", esc(cat.artist_name(&id))));
        }
        h.push_str("</table></div><div><h2>Top tracks</h2><table>\n");
        for (id, n) in top(tracks, 25) {
            let (a, t) = name_of(&id);
            h.push_str(&format!(
                "<tr><td>{} &ndash; <a href=\"https://www.youtube.com/results?search_query={}\" rel=\"noopener\">{}</a></td><td class=\"n\">{n}</td></tr>\n",
                esc(&a), urlenc(&format!("{a} {t}")), esc(&t)
            ));
        }
        h.push_str("</table></div></div>\n");

        h.push_str("<h2>Plays per day</h2><p class=\"meta\">The first and last days may be partial.</p><table>\n");
        for (d, n) in per_day.iter().rev().take(30) {
            h.push_str(&format!("<tr><td>{d}</td><td class=\"n\">{n}</td></tr>\n"));
        }
        h.push_str("</table>\n<h2>Latest plays</h2><table>\n");
        for (at, tid) in plays.iter().rev().take(100) {
            let (a, t) = name_of(tid);
            h.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                esc(at.replace('T', " ").trim_end_matches('Z')), esc(&a), esc(&t)
            ));
        }
        h.push_str("</table>\n");
    }
    h.push_str("<footer><p>Unofficial and non-commercial. Not made, endorsed or checked by Jazz FM or Bauer. \
                Built from the station's public playlist page, read politely a couple of times a day. No ads, no tracking.</p></footer>\n</body></html>\n");
    fs::write(out.join("index.html"), h)
}
