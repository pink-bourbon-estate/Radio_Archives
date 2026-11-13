//! jazzfm: collect Jazz FM's play history into a growing archive, straight from the JSON API the
//! public playlist page itself uses (see docs/JAZZ-FM.md).
//!
//!   jazzfm collect [--days N] [--json FILE] [--data DIR] [--catalogue DIR]
//!   jazzfm import FILE [--data DIR] [--catalogue DIR]   (tab-separated plays collected by hand)
//!   jazzfm export [--data DIR] [--catalogue DIR] [--out DIR]   (plays in the shared site generator's format)
//!   jazzfm stats  [--data DIR] [--catalogue DIR]

mod api;
mod extract;
mod show;
mod export;
mod store;

use catalogue::Catalogue;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::process::exit;
use std::time::{SystemTime, UNIX_EPOCH};

/// How far back to go when nothing is stored yet, and the most a single run will ever go back.
const DEFAULT_DAYS: i64 = 10;

struct Args {
    cmd: String,
    json: Option<PathBuf>,
    days: i64,
    data: PathBuf,
    catalogue: PathBuf,
    out: PathBuf,
    file: Option<PathBuf>,
}

fn parse_args() -> Args {
    let mut it = std::env::args().skip(1);
    let cmd = it.next().unwrap_or_default();
    let mut a = Args {
        cmd,
        json: None,
        days: DEFAULT_DAYS,
        data: "stations/jazz-fm/data".into(),
        catalogue: "catalogue/data".into(),
        out: "public/jazz-fm".into(),
        file: None,
    };
    while let Some(flag) = it.next() {
        let mut val = || it.next().unwrap_or_else(|| die(&format!("{flag} needs a value")));
        match flag.as_str() {
            "--json" => a.json = Some(val().into()),
            "--days" => a.days = val().parse().unwrap_or_else(|_| die("--days needs a whole number")),
            "--data" => a.data = val().into(),
            "--catalogue" => a.catalogue = val().into(),
            "--out" => a.out = val().into(),
            other if !other.starts_with("--") && a.file.is_none() => a.file = Some(other.into()),
            other => die(&format!("unknown option {other}")),
        }
    }
    a
}

fn die(msg: &str) -> ! {
    eprintln!("jazzfm: {msg}");
    exit(2)
}

fn now_utc() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// The newest play already stored, as Unix seconds (None when the archive is empty).
fn newest_stored(a: &Args) -> Option<i64> {
    store::load_all(&a.data)
        .ok()?
        .iter()
        .filter_map(|p| extract::parse_time(&p.0).map(|t| t.0))
        .max()
}

/// Fetch whatever is newer than the archive holds and add it. Run daily: the API keeps about a
/// week of history, so each run asks only for the plays since the previous one.
fn collect(a: &Args) {
    let now = now_utc();
    let floor = now - a.days * 86_400;
    // Overlap by an hour so a play stored a moment ago is never the boundary.
    let stop_at = newest_stored(a).map_or(floor, |t| (t - 3600).max(floor));
    let (plays, note) = match &a.json {
        Some(p) => {
            let text = fs::read_to_string(p).unwrap_or_else(|e| die(&format!("{}: {e}", p.display())));
            (extract::plays_from_json(&text).unwrap_or_else(|e| die(&e)), format!("read from {}", p.display()))
        }
        None => {
            let c = api::collect(now, stop_at).unwrap_or_else(|e| die(&e));
            (c.plays, format!("{} requests; {}", c.requests, c.stopped))
        }
    };
    let plays: Vec<_> = plays.into_iter().filter(|p| p.at > floor).collect();
    if plays.is_empty() {
        die(&format!("found no plays ({note}); has the API changed?"));
    }

    let mut cat = Catalogue::load(&a.catalogue).unwrap_or_else(|e| die(&format!("catalogue: {e}")));
    let (artists_before, tracks_before) = (cat.artists.len(), cat.tracks.len());
    let mut rows = Vec::new();
    let mut skipped = 0;
    for p in &plays {
        match cat.intern(&p.artist, &p.title) {
            Some((_, tid)) => {
                let show = show::show_for(extract::utc_to_london(p.at)).to_string();
                rows.push((p.at, tid, show));
            }
            None => skipped += 1,
        }
    }
    let added = store::merge(&a.data, &rows).unwrap_or_else(|e| die(&format!("store: {e}")));
    cat.save(&a.catalogue).unwrap_or_else(|e| die(&format!("catalogue: {e}")));
    println!(
        "found {} plays ({} to {}); {} new; catalogue +{} artists +{} tracks; {} skipped; {}",
        plays.len(),
        extract::fmt_utc(plays[0].at),
        extract::fmt_utc(plays[plays.len() - 1].at),
        added,
        cat.artists.len() - artists_before,
        cat.tracks.len() - tracks_before,
        skipped,
        note
    );
}

fn unescape(s: &str) -> String {
    s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'")
}

/// Import plays collected by hand: tab-separated `Date, Time_24h, Time_12h, Show, Track, Artist`,
/// with UK local times. Rows for the same track less than 3 minutes apart are one play (the
/// source lists many tracks twice about a minute apart); the later row is kept.
fn import(a: &Args) {
    let path = a.file.as_ref().unwrap_or_else(|| die("import needs a file: jazzfm import FILE"));
    let text = fs::read_to_string(path).unwrap_or_else(|e| die(&format!("{}: {e}", path.display())));
    let text = text.trim_start_matches('\u{feff}');
    let mut cat = Catalogue::load(&a.catalogue).unwrap_or_else(|e| die(&format!("catalogue: {e}")));
    let (artists_before, tracks_before) = (cat.artists.len(), cat.tracks.len());

    // (utc, track id, show in the file)
    let mut rows: Vec<(i64, String, String)> = Vec::new();
    let (mut read, mut bad) = (0, 0);
    for line in text.lines().filter(|l| !l.starts_with("Date\t") && !l.trim().is_empty()) {
        read += 1;
        let f: Vec<&str> = line.split('\t').collect();
        let parsed = (|| {
            if f.len() < 6 {
                return None;
            }
            let date = f[0].get(..10)?;
            let (t, _) = extract::parse_time(&format!("{date} {:0>8}", f[1].trim()))?;
            let (artist, title) = (unescape(f[5]), unescape(f[4]));
            let (_, tid) = cat.intern(&artist, &title)?;
            Some((extract::london_to_utc(t), tid, f[3].trim().to_string()))
        })();
        match parsed {
            Some(r) => rows.push(r),
            None => bad += 1,
        }
    }
    rows.sort();
    let mut kept: Vec<(i64, String, String)> = Vec::new();
    let mut last: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut collapsed = 0;
    for r in rows {
        if let Some(&i) = last.get(&r.1) {
            if r.0 - kept[i].0 <= 180 {
                kept[i] = r.clone();
                collapsed += 1;
                continue;
            }
        }
        last.insert(r.1.clone(), kept.len());
        kept.push(r);
    }

    // Check the timetable against the shows named in the file.
    let mut mismatch: std::collections::BTreeMap<(String, String), usize> = Default::default();
    for (t, _, file_show) in &kept {
        let ours = show::show_for(extract::utc_to_london(*t));
        if ours != file_show {
            *mismatch.entry((ours.to_string(), file_show.clone())).or_default() += 1;
        }
    }
    let with_ours: Vec<(i64, String, String)> = kept
        .iter()
        .map(|(t, id, file_show)| {
            let show = if file_show.is_empty() { show::show_for(extract::utc_to_london(*t)).to_string() } else { file_show.clone() };
            (*t, id.clone(), show)
        })
        .collect();
    let added = store::merge(&a.data, &with_ours).unwrap_or_else(|e| die(&format!("store: {e}")));
    cat.save(&a.catalogue).unwrap_or_else(|e| die(&format!("catalogue: {e}")));

    println!(
        "read {read} rows ({bad} unreadable); {collapsed} repeats within 3 minutes merged; {} plays; {added} new (the rest were already stored)",
        kept.len()
    );
    println!(
        "{} to {}; catalogue +{} artists +{} tracks",
        kept.first().map_or(String::new(), |r| extract::fmt_utc(r.0)),
        kept.last().map_or(String::new(), |r| extract::fmt_utc(r.0)),
        cat.artists.len() - artists_before,
        cat.tracks.len() - tracks_before
    );
    println!("shows in the file that differ from the built-in timetable: {}", mismatch.values().sum::<usize>());
    for ((ours, theirs), n) in mismatch.iter().take(15) {
        println!("  {n:>4}  timetable \"{ours}\"  file \"{theirs}\"");
    }
    let mut gaps = 0;
    for w in kept.windows(2) {
        if w[1].0 - w[0].0 > 1200 {
            gaps += 1;
            println!("  gap over 20 minutes: {} -> {}", extract::fmt_utc(w[0].0), extract::fmt_utc(w[1].0));
        }
    }
    println!("{gaps} gaps over 20 minutes");
}

fn load(a: &Args) -> (Vec<(String, String, String)>, Catalogue) {
    let plays = store::load_all(&a.data).unwrap_or_else(|e| die(&format!("store: {e}")));
    let cat = Catalogue::load(&a.catalogue).unwrap_or_else(|e| die(&format!("catalogue: {e}")));
    (plays, cat)
}

fn main() {
    let a = parse_args();
    match a.cmd.as_str() {
        "collect" => collect(&a),
        "import" => import(&a),
        "export" => {
            let (plays, cat) = load(&a);
            let n = export::write(&plays, &cat, &a.out).unwrap_or_else(|e| die(&format!("export: {e}")));
            println!("wrote {n} plays to {}", a.out.display());
        }
        "stats" => {
            let (plays, cat) = load(&a);
            let tracks: HashSet<&str> = plays.iter().map(|p| p.1.as_str()).collect();
            println!(
                "{} plays, {} distinct tracks, {} artists in the catalogue ({} tracks)",
                plays.len(),
                tracks.len(),
                cat.artists.len(),
                cat.tracks.len()
            );
        }
        _ => die("usage: jazzfm collect|import|export|stats [options]"),
    }
}
