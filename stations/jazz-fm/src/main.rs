//! jazzfm: collect Jazz FM's public playlist into a growing archive.
//!
//!   jazzfm scrape [--html FILE] [--url URL] [--data DIR] [--catalogue DIR] [--dump FILE]
//!   jazzfm site   [--data DIR] [--catalogue DIR] [--out DIR]
//!   jazzfm stats  [--data DIR] [--catalogue DIR]

mod extract;
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
            Some((_, tid)) => rows.push((p.at, tid)),
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

fn load(a: &Args) -> (Vec<(String, String)>, Catalogue) {
    let plays = store::load_all(&a.data).unwrap_or_else(|e| die(&format!("store: {e}")));
    let cat = Catalogue::load(&a.catalogue).unwrap_or_else(|e| die(&format!("catalogue: {e}")));
    (plays, cat)
}

fn main() {
    let a = parse_args();
    match a.cmd.as_str() {
        "scrape" => scrape(&a),
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
