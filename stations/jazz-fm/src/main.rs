//! jazzfm: collect Jazz FM's public playlist into a growing archive.
//!
//!   jazzfm scrape [--html FILE] [--url URL] [--data DIR] [--catalogue DIR] [--dump FILE]
//!   jazzfm import FILE [--data DIR] [--catalogue DIR]   (tab-separated plays collected by hand)
//!   jazzfm site   [--data DIR] [--catalogue DIR] [--out DIR]
//!   jazzfm stats  [--data DIR] [--catalogue DIR]

mod extract;
mod show;
mod site;
mod store;

use catalogue::Catalogue;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::process::exit;
use std::time::Duration;

const DEFAULT_URL: &str = "https://www.hellorayo.co.uk/jazz-fm/playlist";
const USER_AGENT: &str = "RadioArchivesBot/0.1 (+https://github.com/pink-bourbon-estate/Radio_Archives; \
                          unofficial non-commercial archive; low request rate)";

struct Args {
    cmd: String,
    html: Option<PathBuf>,
    url: String,
    data: PathBuf,
    catalogue: PathBuf,
    dump: Option<PathBuf>,
    out: PathBuf,
    file: Option<PathBuf>,
}

fn parse_args() -> Args {
    let mut it = std::env::args().skip(1);
    let cmd = it.next().unwrap_or_default();
    let mut a = Args {
        cmd,
        html: None,
        url: DEFAULT_URL.into(),
        data: "stations/jazz-fm/data".into(),
        catalogue: "catalogue/data".into(),
        dump: None,
        out: "public/jazz-fm".into(),
        file: None,
    };
    while let Some(flag) = it.next() {
        let mut val = || it.next().unwrap_or_else(|| die(&format!("{flag} needs a value")));
        match flag.as_str() {
            "--html" => a.html = Some(val().into()),
            "--url" => a.url = val(),
            "--data" => a.data = val().into(),
            "--catalogue" => a.catalogue = val().into(),
            "--dump" => a.dump = Some(val().into()),
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

fn host_and_path(url: &str) -> (String, String) {
    let rest = url.split("://").nth(1).unwrap_or(url);
    match rest.find('/') {
        Some(i) => (rest[..i].to_string(), rest[i..].to_string()),
        None => (rest.to_string(), "/".to_string()),
    }
}

fn fetch(url: &str) -> String {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(30))
        .user_agent(USER_AGENT)
        .build();
    // Be polite: honour robots.txt before asking for the page.
    let (host, path) = host_and_path(url);
    let scheme = if url.starts_with("http://") { "http" } else { "https" };
    match agent.get(&format!("{scheme}://{host}/robots.txt")).call() {
        Ok(r) => {
            let body = r.into_string().unwrap_or_default();
            if !extract::robots_allows(&body, &path) {
                die(&format!("robots.txt on {host} disallows {path} for us; not fetching"));
            }
        }
        Err(e) => eprintln!("note: could not read robots.txt ({e}); carrying on"),
    }
    std::thread::sleep(Duration::from_secs(2));
    match agent.get(url).call() {
        Ok(r) => r.into_string().unwrap_or_else(|e| die(&format!("could not read the page: {e}"))),
        Err(e) => die(&format!("could not fetch {url}: {e}")),
    }
}

fn scrape(a: &Args) {
    let html = match &a.html {
        Some(p) => fs::read_to_string(p).unwrap_or_else(|e| die(&format!("{}: {e}", p.display()))),
        None => fetch(&a.url),
    };
    let fail = |why: String| -> ! {
        if let Some(d) = &a.dump {
            let _ = fs::write(d, &html);
            eprintln!("saved the page to {} for inspection", d.display());
        }
        die(&why)
    };
    let Some(json) = extract::next_data(&html) else {
        fail("no __NEXT_DATA__ block in the page (has the site changed?)".into())
    };
    let ex = extract::extract(json).unwrap_or_else(|e| fail(e));
    if ex.plays.is_empty() {
        fail(format!("found no plays in the page data. {}", extract::describe(json)));
    }
    // The page only embeds the latest few plays, so plays are lost if this runs too rarely.
    if ex.plays.len() < 3 {
        eprintln!("warning: only {} plays found", ex.plays.len());
    }

    let mut cat = Catalogue::load(&a.catalogue).unwrap_or_else(|e| die(&format!("catalogue: {e}")));
    let (artists_before, tracks_before) = (cat.artists.len(), cat.tracks.len());
    let mut rows = Vec::new();
    let mut skipped = 0;
    for p in &ex.plays {
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
        "found {} plays ({} to {}); {} new; catalogue +{} artists +{} tracks; {} skipped",
        ex.plays.len(),
        extract::fmt_utc(ex.plays[0].at),
        extract::fmt_utc(ex.plays[ex.plays.len() - 1].at),
        added,
        cat.artists.len() - artists_before,
        cat.tracks.len() - tracks_before,
        skipped
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
        "scrape" => scrape(&a),
        "import" => import(&a),
        "site" => {
            let (plays, cat) = load(&a);
            let noindex = std::env::var("SITE_NOINDEX").is_ok_and(|v| !v.is_empty() && v != "0");
            site::build(&plays, &cat, &a.out, noindex).unwrap_or_else(|e| die(&format!("site: {e}")));
            println!("wrote {} ({} plays)", a.out.display(), plays.len());
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
        _ => die("usage: jazzfm scrape|site|stats [options]"),
    }
}
