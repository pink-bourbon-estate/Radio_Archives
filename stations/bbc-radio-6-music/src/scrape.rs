//! Fetch the BBC's public schedule and tracklist data and store every play.
//!
//! Day -> episodes:  https://www.bbc.co.uk/schedules/<service>/<yyyy>/<mm>/<dd>
//!   (the page embeds a JSON-LD "@graph" of RadioEpisode objects)
//! Episode -> tracks: https://www.bbc.co.uk/programmes/<pid>/segments.json
//!
//! Every response is cached on disk, so stopping and restarting costs nothing and
//! finished episodes (listed in episodes.csv) are never fetched twice.

use crate::util::*;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const BBC: &str = "https://www.bbc.co.uk";
pub const DEFAULT_SERVICE: &str = "p00fzl65"; // BBC Radio 6 Music
/// Say who is asking. The program names itself and links to its code, so the BBC can see
/// what is making the requests and get in touch (it does not pretend to be a web browser).
const UA: &str = concat!(
    "bbc6-radio6-collection/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/pink-bourbon-estate/bbc-radio-6-music-collection)"
);

pub const PLAYS_HEADER: [&str; 14] = [
    "Date",
    "Time",
    "Show",
    "Episode",
    "EpisodePID",
    "Position",
    "OffsetSec",
    "Artist",
    "Title",
    "Album",
    "Label",
    "Publisher",
    "DurationSec",
    "MusicBrainzArtistID",
];
pub const EPISODES_HEADER: [&str; 6] = ["Date", "Time", "Show", "Episode", "EpisodePID", "Tracks"];

// ---------------------------------------------------------------- fetching

pub struct Fetcher {
    pub base: String,
    agent: ureq::Agent,
    cache: PathBuf,
    gap: Duration,
    offline: bool,
    last: Mutex<Instant>,
    pub requests: AtomicUsize,
    pub cache_hits: AtomicUsize,
    pub errors: AtomicUsize,
}

impl Fetcher {
    pub fn new(base: &str, cache: &Path, gap_secs: f64, offline: bool) -> Fetcher {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(5))
            .timeout_read(Duration::from_secs(30))
            .user_agent(UA)
            .build();
        Fetcher {
            base: base.trim_end_matches('/').to_string(),
            agent,
            cache: cache.to_path_buf(),
            gap: Duration::from_secs_f64(gap_secs.max(0.0)),
            offline,
            last: Mutex::new(Instant::now() - Duration::from_secs(60)),
            requests: AtomicUsize::new(0),
            cache_hits: AtomicUsize::new(0),
            errors: AtomicUsize::new(0),
        }
    }

    /// Where a URL's response is cached. Same layout as the original Python tool,
    /// so an existing `_cache` folder keeps working.
    fn cache_path(&self, url: &str) -> PathBuf {
        let rel = url.strip_prefix(&format!("{}/", self.base)).unwrap_or(url);
        let mut rel: String = rel
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || "._/-".contains(c) {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        if rel.ends_with('/') {
            rel.push_str("index");
        }
        self.cache.join(rel)
    }

    /// GET a URL as text (cache first). `None` means absent (404) or unavailable.
    pub fn get(&self, url: &str) -> Option<String> {
        let path = self.cache_path(url);
        if let Ok(text) = fs::read_to_string(&path) {
            if text == "__404__" {
                return None;
            }
            self.cache_hits.fetch_add(1, Ordering::Relaxed);
            return Some(text);
        }
        if self.offline {
            return None;
        }
        for attempt in 0..4u32 {
            self.wait_turn();
            self.requests.fetch_add(1, Ordering::Relaxed);
            match self.agent.get(url).call() {
                Ok(resp) => {
                    let mut body = String::new();
                    if resp
                        .into_reader()
                        .take(64 << 20)
                        .read_to_string(&mut body)
                        .is_err()
                    {
                        self.errors.fetch_add(1, Ordering::Relaxed);
                        return None;
                    }
                    write_atomic(&path, &body);
                    return Some(body);
                }
                Err(ureq::Error::Status(404, _)) => {
                    write_atomic(&path, "__404__"); // remember, so we never ask again
                    return None;
                }
                Err(ureq::Error::Status(code, _))
                    if attempt < 3 && matches!(code, 429 | 500 | 502 | 503 | 504) => {}
                Err(ureq::Error::Transport(_)) if attempt < 3 => {}
                Err(e) => {
                    self.errors.fetch_add(1, Ordering::Relaxed);
                    eprintln!("[warn] {url}: {e}");
                    return None;
                }
            }
            std::thread::sleep(Duration::from_millis(
                (1500.0 * 2f64.powi(attempt as i32)) as u64,
            ));
        }
        self.errors.fetch_add(1, Ordering::Relaxed);
        eprintln!("[warn] {url}: gave up after retries");
        None
    }

    /// Keep requests at least `gap` apart across all threads.
    fn wait_turn(&self) {
        let mut last = self.last.lock().unwrap();
        let since = last.elapsed();
        if since < self.gap {
            std::thread::sleep(self.gap - since);
        }
        *last = Instant::now();
    }
}

fn write_atomic(path: &Path, body: &str) {
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("part");
    if fs::write(&tmp, body).is_ok() {
        let _ = fs::rename(&tmp, path);
    }
}

// ----------------------------------------------------------------- parsing

#[derive(Clone, Debug, Default)]
pub struct Episode {
    pub pid: String,
    pub show: String,
    pub episode: String,
    pub date: String,
    pub time: String,
}

#[derive(Debug, Default, PartialEq)]
pub struct Track {
    pub position: String,
    pub offset: String,
    pub artist: String,
    pub title: String,
    pub album: String,
    pub label: String,
    pub publisher: String,
    pub duration: String,
    pub mbid: String,
}

fn text(v: &Value, key: &str) -> String {
    scrub_emails(v.get(key).and_then(|x| x.as_str()).unwrap_or("").trim()).into_owned()
}

/// A JSON number or string as text; zero, null and missing all become "".
fn numstr(v: Option<&Value>) -> String {
    match v {
        Some(Value::Number(n)) if n.as_f64() != Some(0.0) => n.to_string(),
        Some(Value::String(s)) if !s.is_empty() && s != "0" => s.clone(),
        _ => String::new(),
    }
}

/// Pull the RadioEpisode list out of a schedule page's JSON-LD block.
pub fn parse_schedule(html: &str, fallback_date: &str) -> Vec<Episode> {
    let mut rest = html;
    while let Some(i) = rest.find("application/ld+json") {
        rest = &rest[i..];
        let (Some(open), Some(close)) = (rest.find('>'), rest.find("</script>")) else {
            break;
        };
        if close < open {
            break;
        }
        let block = &rest[open + 1..close];
        rest = &rest[close..];
        let Ok(data) = serde_json::from_str::<Value>(block.trim()) else {
            continue;
        };
        let Some(graph) = data.get("@graph").and_then(|g| g.as_array()) else {
            continue;
        };

        let mut out = Vec::new();
        for e in graph {
            if let Some(t) = e.get("@type").and_then(|t| t.as_str()) {
                if !matches!(t, "RadioEpisode" | "TVEpisode" | "Episode") {
                    continue;
                }
            }
            let pid = text(e, "identifier");
            if pid.is_empty() {
                continue;
            }
            let series = e.get("partOfSeries").cloned().unwrap_or(Value::Null);
            let start = e
                .get("publication")
                .map(|p| text(p, "startDate"))
                .unwrap_or_default();
            let date = if start.len() >= 10 && start.is_char_boundary(10) {
                start[..10].to_string()
            } else {
                let d = text(e, "datePublished");
                if d.is_empty() {
                    fallback_date.to_string()
                } else {
                    d
                }
            };
            out.push(Episode {
                pid,
                show: text(&series, "name"),
                episode: text(e, "name"),
                date,
                time: if start.len() >= 16 && start.is_char_boundary(16) {
                    start[11..16].to_string()
                } else {
                    String::new()
                },
            });
        }
        return out;
    }
    Vec::new()
}

/// The music tracks in one episode's segments.json, in broadcast order.
pub fn parse_segments(body: &str) -> Vec<Track> {
    let Ok(data) = serde_json::from_str::<Value>(body) else {
        return Vec::new();
    };
    let Some(events) = data.get("segment_events").and_then(|e| e.as_array()) else {
        return Vec::new();
    };
    let mut out: Vec<Track> = Vec::new();
    for ev in events {
        if ev
            .get("is_chapter")
            .and_then(|c| c.as_bool())
            .unwrap_or(false)
        {
            continue;
        }
        let Some(seg) = ev.get("segment") else {
            continue;
        };
        if seg.get("type").and_then(|t| t.as_str()) != Some("music") {
            continue;
        }
        let pc = seg
            .get("primary_contributor")
            .cloned()
            .unwrap_or(Value::Null);
        let mut artist = text(seg, "artist");
        if artist.is_empty() {
            artist = text(&pc, "name");
        }
        let mut title = text(seg, "track_title");
        if title.is_empty() {
            title = text(seg, "title");
        }
        if artist.is_empty() && title.is_empty() {
            continue;
        }
        let mut position = numstr(ev.get("position"));
        if position.is_empty() {
            position = (out.len() + 1).to_string();
        }
        out.push(Track {
            position,
            offset: numstr(ev.get("version_offset")),
            artist,
            title,
            album: text(seg, "release_title"),
            label: text(seg, "record_label"),
            publisher: text(seg, "publisher"),
            duration: numstr(seg.get("duration")),
            mbid: text(&pc, "musicbrainz_gid"),
        });
    }
    out
}

pub fn episodes_for_day(f: &Fetcher, service: &str, d: Day) -> Vec<Episode> {
    let (y, m, dd) = d.ymd();
    match f.get(&format!(
        "{}/schedules/{service}/{y:04}/{m:02}/{dd:02}",
        f.base
    )) {
        Some(html) => parse_schedule(&html, &d.iso()),
        None => Vec::new(),
    }
}

pub fn tracks_for_episode(f: &Fetcher, pid: &str) -> Vec<Track> {
    f.get(&format!("{}/programmes/{pid}/segments.json", f.base))
        .map(|b| parse_segments(&b))
        .unwrap_or_default()
}

// ---------------------------------------------------------------- commands

pub fn probe(args: &Args) -> Res<()> {
    let data = args.str_or("data", "data");
    let day = Day::parse(&args.str_or("date", "2016-03-17"))?;
    let service = args.str_or("service", DEFAULT_SERVICE);
    let f = Fetcher::new(
        &args.str_or("base-url", BBC),
        &Path::new(&data).join("_cache"),
        args.num("gap", 0.25)?,
        args.flag("offline"),
    );

    println!("[1/2] Schedule page for {}...", day.iso());
    let eps = episodes_for_day(&f, &service, day);
    if eps.is_empty() {
        return Err("no episodes parsed from the schedule page (blocked, offline, or the page layout changed)".into());
    }
    println!("      OK - {} episodes. First few:", eps.len());
    for e in eps.iter().take(5) {
        println!(
            "        {:>5}  {:<34} | {} [{}]",
            e.time,
            e.show.chars().take(34).collect::<String>(),
            e.episode,
            e.pid
        );
    }
    let target = eps
        .iter()
        .find(|e| e.time.as_str() >= "10:00")
        .unwrap_or(&eps[0]);
    println!("\n[2/2] Tracklist for {} [{}]...", target.show, target.pid);
    let tracks = tracks_for_episode(&f, &target.pid);
    if tracks.is_empty() {
        return Err(
            "no tracks for that episode (it may genuinely have none - try another date)".into(),
        );
    }
    println!("      OK - {} tracks. First few:", tracks.len());
    for t in tracks.iter().take(8) {
        println!("        {:>3}. {} - {}", t.position, t.artist, t.title);
    }
    println!("\nPROBE PASSED.");
    Ok(())
}

/// Append-only CSV file that writes its header when first created.
struct Appender {
    w: csv::Writer<File>,
}

impl Appender {
    fn open(path: &Path, header: &[&str]) -> Res<Appender> {
        let fresh = fs::metadata(path).map(|m| m.len() == 0).unwrap_or(true);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(err("cannot create folder"))?;
        }
        let f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(err(&format!("cannot open {}", path.display())))?;
        let mut w = csv::Writer::from_writer(f);
        if fresh {
            w.write_record(header).map_err(err("write"))?;
        }
        Ok(Appender { w })
    }
}

/// Episodes already recorded in episodes.csv: pid -> number of tracks found.
fn load_ledger(path: &Path) -> HashMap<String, u64> {
    let mut m = HashMap::new();
    if let Ok(mut r) = open_csv(path) {
        if let Ok(c) = Cols::of(&mut r, path) {
            for rec in r.records().flatten() {
                m.insert(
                    c.get(&rec, "EpisodePID").to_string(),
                    parse_plays(c.get(&rec, "Tracks")),
                );
            }
        }
    }
    m
}

pub fn run(args: &Args) -> Res<()> {
    let data = PathBuf::from(args.str_or("data", "data"));
    let service = args.str_or("service", DEFAULT_SERVICE);
    let start = Day::parse(&args.str_or("start", "2007-01-01"))?;
    let end = match args.get("end") {
        Some(e) => Day::parse(e)?,
        None => Day(Day::today().0 - 1),
    };
    if end < start {
        return Err("--end is before --start".into());
    }
    let workers: usize = args.num("workers", 3usize)?.clamp(1, 6);
    let gap: f64 = args.num("gap", 0.25)?;
    let offline = args.flag("offline");
    let retry_empty = args.flag("retry-empty");

    let f = Fetcher::new(
        &args.str_or("base-url", BBC),
        &data.join("_cache"),
        gap,
        offline,
    );
    let ledger_path = data.join("episodes.csv");
    let mut ledger = load_ledger(&ledger_path);
    let mut eps_out = Appender::open(&ledger_path, &EPISODES_HEADER)?;
    let mut plays_out: HashMap<String, Appender> = HashMap::new();

    println!("Data folder: {}", data.display());
    println!(
        "Range:       {} to {} ({} days)",
        start.iso(),
        end.iso(),
        end.0 - start.0 + 1
    );
    println!("Episodes already recorded: {}\n", ledger.len());

    let mut seen: HashSet<String> = HashSet::new();
    let (mut n_days, mut n_eps, mut n_plays) = (0usize, 0usize, 0usize);
    let mut day = start;
    while day <= end {
        let todo: Vec<Episode> = episodes_for_day(&f, &service, day)
            .into_iter()
            .filter(|e| seen.insert(e.pid.clone())) // schedule days overlap at midnight
            .filter(|e| match ledger.get(&e.pid) {
                None => true,
                Some(&n) => retry_empty && n == 0,
            })
            .collect();

        if !todo.is_empty() {
            let mut results: Vec<Vec<Track>> = (0..todo.len()).map(|_| Vec::new()).collect();
            std::thread::scope(|s| {
                let handles: Vec<_> = (0..workers.min(todo.len()))
                    .map(|w| {
                        let (f, todo) = (&f, &todo);
                        s.spawn(move || {
                            (w..todo.len())
                                .step_by(workers)
                                .map(|i| (i, tracks_for_episode(f, &todo[i].pid)))
                                .collect::<Vec<_>>()
                        })
                    })
                    .collect();
                for h in handles {
                    for (i, t) in h.join().unwrap_or_default() {
                        results[i] = t;
                    }
                }
            });

            let mut day_tracks = 0;
            for (e, tracks) in todo.iter().zip(&results) {
                for t in tracks {
                    let year = e.date.get(..4).unwrap_or("0000").to_string();
                    if !plays_out.contains_key(&year) {
                        let path = data.join("plays").join(format!("plays_{year}.csv"));
                        plays_out.insert(year.clone(), Appender::open(&path, &PLAYS_HEADER)?);
                    }
                    let w = &mut plays_out.get_mut(&year).unwrap().w;
                    w.write_record([
                        &e.date,
                        &e.time,
                        &e.show,
                        &e.episode,
                        &e.pid,
                        &t.position,
                        &t.offset,
                        &t.artist,
                        &t.title,
                        &t.album,
                        &t.label,
                        &t.publisher,
                        &t.duration,
                        &t.mbid,
                    ])
                    .map_err(err("write"))?;
                }
                if ledger.get(&e.pid) != Some(&(tracks.len() as u64)) {
                    // new episode, or a retried one whose track count changed
                    eps_out
                        .w
                        .write_record([
                            &e.date,
                            &e.time,
                            &e.show,
                            &e.episode,
                            &e.pid,
                            &tracks.len().to_string(),
                        ])
                        .map_err(err("write"))?;
                }
                ledger.insert(e.pid.clone(), tracks.len() as u64);
                day_tracks += tracks.len();
            }
            // Flush plays before the ledger, so a crash can only cause a harmless re-fetch.
            for a in plays_out.values_mut() {
                a.w.flush().map_err(err("flush"))?;
            }
            eps_out.w.flush().map_err(err("flush"))?;
            n_eps += todo.len();
            n_plays += day_tracks;
            println!(
                "[{}] {:>2} episodes, {:>4} tracks (total {})",
                day.iso(),
                todo.len(),
                day_tracks,
                n_plays
            );
        }
        n_days += 1;
        day = Day(day.0 + 1);
    }

    println!("\nDone. {n_days} days, {n_eps} new episodes, {n_plays} new plays.");
    println!(
        "Network requests: {}, cache hits: {}, errors: {}",
        f.requests.load(Ordering::Relaxed),
        f.cache_hits.load(Ordering::Relaxed),
        f.errors.load(Ordering::Relaxed)
    );
    if n_plays > 0 {
        println!("Next: run `bbc6 derive` to rebuild the song database.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCHEDULE: &str = r#"<html><head><script type="application/ld+json">{"@context":"x","other":1}</script>
<script type="application/ld+json">{"@graph":[
 {"@type":"RadioEpisode","identifier":"b07abcde","name":"The Magnetic North","partOfSeries":{"name":"Marc Riley","identifier":"b00s1"},"publication":{"startDate":"2016-03-17T21:00:00+00:00"}},
 {"@type":"RadioEpisode","identifier":"","name":"no pid"},
 {"@type":"Organization","identifier":"zzz"}
]}</script></head></html>"#;

    #[test]
    fn schedule_is_parsed() {
        let eps = parse_schedule(SCHEDULE, "2000-01-01");
        assert_eq!(eps.len(), 1);
        assert_eq!(eps[0].pid, "b07abcde");
        assert_eq!(eps[0].show, "Marc Riley");
        assert_eq!(eps[0].date, "2016-03-17");
        assert_eq!(eps[0].time, "21:00");
        assert!(parse_schedule("<html>nothing</html>", "x").is_empty());
    }

    #[test]
    fn segments_keep_only_music() {
        let body = r#"{"segment_events":[
 {"position":1,"version_offset":12,"segment":{"type":"music","artist":"Roxy Music","track_title":"Love Is the Drug","release_title":"Siren","record_label":"Island","duration":242,
   "primary_contributor":{"name":"Roxy Music","musicbrainz_gid":"331ce348-1b08-40b9-8ed7-0763b92bd003"}}},
 {"position":2,"segment":{"type":"speech","title":"chat"}},
 {"is_chapter":true,"segment":{"type":"music","artist":"X","track_title":"Y"}},
 {"segment":{"type":"music","primary_contributor":{"name":"Someone"},"title":"Fallback Title"}}
]}"#;
        let t = parse_segments(body);
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].artist, "Roxy Music");
        assert_eq!(t[0].duration, "242");
        assert_eq!(t[0].mbid, "331ce348-1b08-40b9-8ed7-0763b92bd003");
        assert_eq!(t[1].artist, "Someone");
        assert_eq!(t[1].title, "Fallback Title");
        assert_eq!(t[1].position, "2");
        assert!(parse_segments("not json").is_empty());
    }
}
