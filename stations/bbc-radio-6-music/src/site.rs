//! Build the website's data files.
//!
//! The website (in `site/`) is plain HTML, CSS and JavaScript. It cannot read a 100 MB CSV in a
//! browser, so this command pre-computes small JSON files from the plays and the artist data:
//!
//! ```text
//! <out>/data/meta.json          when the data ends, totals
//! <out>/data/summary.json       plays per year / weekday / hour, top shows, genres, countries
//! <out>/data/windows.json       most played today, this week, this month, this year
//! <out>/data/latest.json        every episode of the last 7 days with its tracklist
//! <out>/data/years.json         the top tracks and artists of every year
//! <out>/data/tracks_top.json    the most-played tracks of all time
//! <out>/data/tracks_rare.json   one-off plays, newest first
//! <out>/data/crate.json         one-off tracks and artists counted for every year
//! <out>/data/crate/<year>.json  that year's one-off tracks and artists
//! <out>/data/artists_top.json   the most-played artists of all time
//! <out>/data/shows.json         every show
//! <out>/data/shows/<slug>.json  one show: all its tracks, top artists, latest episodes
//! <out>/data/artists/<a-z>.json every artist starting with that letter, with MusicBrainz info
//! ```
//!
//! "Today" and "this week" mean the days up to the **last day in the data**, not the clock on
//! your computer, because the data only ever ends on the last day that was scraped.

use crate::derive::plays_files;
use crate::pages::{esc, listen_links, Emitter};
use crate::playlists::classify;
use crate::util::*;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

const ARTIST_PAGES: usize = 5000;
/// Rows kept for each year of the crate dive, spread evenly across that year.
const CRATE_ROWS: usize = 1000;
const REPO: &str = "https://github.com/pink-bourbon-estate/bbc-radio-6-music-collection";

/// One play, stored compactly (everything is an index into a table).
struct Row {
    day: i64,
    ti: u32,
    ai: u32, // u32::MAX when the artist is blank
    si: u32,
    ep: u32,
}

struct Track {
    artist: String,
    title: String,
    ai: u32,
    plays: u32,
    first: i64,
    last: i64,
}

struct Artist {
    name: String,
    mbid: String,
    plays: u32,
    tracks: u32,
    first: i64,
    last: i64,
    id: String,
    letter: String,
}

struct Show {
    name: String,
    slug: String,
}

struct Episode {
    date: String,
    time: String,
    show: u32,
    title: String,
    pid: String,
    /// (artist name, title, artist index) in broadcast order
    tracks: Vec<(String, String, u32)>,
}

/// MusicBrainz details for one artist (from artists_enriched.csv).
#[derive(Default, Clone)]
struct Mb {
    genre: String,
    genres: String,
    country: String,
    begin: String,
    kind: String,
}

/// A-Z shard for an artist, ignoring a leading "The ".
fn letter_of(name: &str) -> String {
    let n = norm(name);
    let n = n.strip_prefix("the ").unwrap_or(&n);
    match n.chars().find(|c| c.is_alphanumeric()) {
        Some(c) if c.is_ascii_alphabetic() => c.to_ascii_lowercase().to_string(),
        Some(c) if c.is_ascii_digit() => "0".into(),
        _ => "other".into(),
    }
}

/// Lowercase ASCII letters and digits separated by single dashes.
fn slug(s: &str) -> String {
    let mut out = String::new();
    let mut dash = true;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !dash {
            out.push('-');
            dash = true;
        }
    }
    let out = out.trim_matches('-').to_string();
    out.chars().take(60).collect::<String>().trim_matches('-').to_string()
}

/// Make `base` unique among `used` by adding -2, -3 ...
fn unique(base: String, used: &mut HashSet<String>) -> String {
    let base = if base.is_empty() { "x".to_string() } else { base };
    let mut candidate = base.clone();
    let mut n = 2;
    while !used.insert(candidate.clone()) {
        candidate = format!("{base}-{n}");
        n += 1;
    }
    candidate
}

fn write_json(path: &Path, v: &Value) -> Res<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(err("cannot create folder"))?;
    }
    let f = fs::File::create(path).map_err(err(&format!("cannot write {}", path.display())))?;
    serde_json::to_writer(std::io::BufWriter::new(f), v).map_err(err("json"))
}

/// 1234567 -> "1,234,567"
fn thou(n: u64) -> String {
    let s = n.to_string();
    let mut o = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            o.push(',');
        }
        o.push(c);
    }
    o
}

fn iso(day: i64) -> String {
    Day(day).iso()
}

/// The `n` biggest entries of a count map, biggest first (ties by index so output is stable).
fn top(counts: &HashMap<u32, u32>, n: usize) -> Vec<(u32, u32)> {
    let mut v: Vec<(u32, u32)> = counts.iter().map(|(k, c)| (*k, *c)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v.truncate(n);
    v
}

fn load_mb(data: &Path) -> HashMap<String, Mb> {
    let mut map = HashMap::new();
    let path = data.join("artists_enriched.csv");
    let Ok(mut r) = open_csv(&path) else {
        return map;
    };
    let Ok(c) = Cols::of(&mut r, &path) else {
        return map;
    };
    for rec in r.records().flatten() {
        let id = c.get(&rec, "MusicBrainzArtistID");
        if id.is_empty() {
            continue;
        }
        map.insert(
            id.to_string(),
            Mb {
                genre: scrub_emails(c.get(&rec, "PrimaryGenre")).into_owned(),
                genres: scrub_emails(c.get(&rec, "Genres")).into_owned(),
                country: c.get(&rec, "Country").to_string(),
                begin: c.get(&rec, "BeginYear").to_string(),
                kind: c.get(&rec, "Type").to_string(),
            },
        );
    }
    map
}

pub fn run(args: &Args) -> Res<()> {
    let data = PathBuf::from(args.str_or("data", "data"));
    let src = PathBuf::from(args.str_or("src", "site"));
    let root = PathBuf::from(args.str_or("out", "public"));
    let out = root.join("data");
    let origin = args.str_or("origin", "https://example.github.io");
    let base = args.str_or("base", "/");
    let files = plays_files(&data, true);
    if files.is_empty() {
        return Err(format!("no plays files in {}/plays", data.display()));
    }

    // ---------------------------------------------------------------- read
    let mut tidx: HashMap<(String, String), u32> = HashMap::new();
    let mut tracks: Vec<Track> = Vec::new();
    let mut aidx: HashMap<String, u32> = HashMap::new();
    let mut artists: Vec<Artist> = Vec::new();
    let mut sidx: HashMap<String, u32> = HashMap::new();
    let mut shows: Vec<Show> = Vec::new();
    let mut eidx: HashMap<String, u32> = HashMap::new();
    let mut episodes: Vec<Episode> = Vec::new();
    let mut hours: Vec<u8> = Vec::new(); // per episode
    let mut rows: Vec<Row> = Vec::new();

    for path in &files {
        println!("Reading {} ...", path.display());
        let mut r = open_csv(path)?;
        let c = Cols::of(&mut r, path)?;
        for rec in r.records().flatten() {
            let artist = scrub_emails(c.get(&rec, "Artist")).into_owned();
            let title = scrub_emails(c.get(&rec, "Title")).into_owned();
            let (na, nt) = (norm(&artist), norm(&title));
            if na.is_empty() && nt.is_empty() {
                continue;
            }
            let date = c.get(&rec, "Date");
            let Ok(day) = Day::parse(date) else { continue };
            let show_name = scrub_emails(c.get(&rec, "Show")).into_owned();
            let show_name = if show_name.trim().is_empty() { "Unknown Show".to_string() } else { show_name };
            let mbid = c.get(&rec, "MusicBrainzArtistID");

            let si = *sidx.entry(show_name.clone()).or_insert_with(|| {
                shows.push(Show { name: show_name.clone(), slug: String::new() });
                (shows.len() - 1) as u32
            });

            let pid = c.get(&rec, "EpisodePID");
            let ekey = if pid.is_empty() { format!("{date}|{}|{show_name}", c.get(&rec, "Time")) } else { pid.to_string() };
            let ep = *eidx.entry(ekey).or_insert_with(|| {
                episodes.push(Episode {
                    date: date.to_string(),
                    time: c.get(&rec, "Time").to_string(),
                    show: si,
                    title: scrub_emails(c.get(&rec, "Episode")).into_owned(),
                    pid: pid.to_string(),
                    tracks: Vec::new(),
                });
                hours.push(c.get(&rec, "Time").get(..2).and_then(|h| h.parse().ok()).unwrap_or(0));
                (episodes.len() - 1) as u32
            });

            let ai = if na.is_empty() {
                u32::MAX
            } else {
                let i = *aidx.entry(na.clone()).or_insert_with(|| {
                    artists.push(Artist {
                        name: artist.clone(),
                        mbid: mbid.to_string(),
                        plays: 0,
                        tracks: 0,
                        first: day.0,
                        last: day.0,
                        id: String::new(),
                        letter: String::new(),
                    });
                    (artists.len() - 1) as u32
                });
                let a = &mut artists[i as usize];
                a.plays += 1;
                a.first = a.first.min(day.0);
                a.last = a.last.max(day.0);
                if a.mbid.is_empty() {
                    a.mbid = mbid.to_string();
                }
                i
            };

            let ti = *tidx.entry((na, nt)).or_insert_with(|| {
                tracks.push(Track { artist: artist.clone(), title: title.clone(), ai, plays: 0, first: day.0, last: day.0 });
                if ai != u32::MAX {
                    artists[ai as usize].tracks += 1;
                }
                (tracks.len() - 1) as u32
            });
            let t = &mut tracks[ti as usize];
            t.plays += 1;
            t.first = t.first.min(day.0);
            t.last = t.last.max(day.0);

            episodes[ep as usize].tracks.push((artist, title, ai));
            rows.push(Row { day: day.0, ti, ai, si, ep });
        }
    }
    if rows.is_empty() {
        return Err("no plays were read".into());
    }
    let latest = rows.iter().map(|r| r.day).max().unwrap_or(0);
    let earliest = rows.iter().map(|r| r.day).min().unwrap_or(0);
    println!("{} plays, {} tracks, {} artists, {} shows, last day {}", rows.len(), tracks.len(), artists.len(), shows.len(), iso(latest));

    // ids and slugs
    let mut used = HashSet::new();
    for s in shows.iter_mut() {
        s.slug = unique(slug(&s.name), &mut used);
    }
    let mut used = HashSet::new();
    for a in artists.iter_mut() {
        a.id = unique(slug(&a.name), &mut used);
        a.letter = letter_of(&a.name);
    }
    let mb = load_mb(&data);
    let mb_of = |a: &Artist| mb.get(&a.mbid).cloned().unwrap_or_default();
    let mut aorder: Vec<u32> = (0..artists.len() as u32).collect();
    aorder.sort_by(|&a, &b| artists[b as usize].plays.cmp(&artists[a as usize].plays).then(a.cmp(&b)));
    // the artists that get their own pre-built page (the app opens every other artist too)
    let page_ids: HashSet<String> = aorder.iter().take(ARTIST_PAGES).map(|&a| artists[a as usize].id.clone()).collect();
    let mut em = Emitter::new(&src, &root, &origin, &base, &iso(latest))?;

    // a track row for tables: [artist, title, plays, artist id, artist letter]
    let trow = |ti: u32, plays: u32| -> Value {
        let t = &tracks[ti as usize];
        if t.ai == u32::MAX {
            json!([t.artist, t.title, plays, "", ""])
        } else {
            let a = &artists[t.ai as usize];
            json!([t.artist, t.title, plays, a.id, a.letter])
        }
    };
    let arow = |ai: u32, plays: u32| -> Value {
        let a = &artists[ai as usize];
        json!([a.name, a.id, a.letter, plays])
    };

    // ----------------------------------------------------- per-year counts
    let year_of = |day: i64| Day(day).ymd().0;
    let mut years: Vec<i64> = rows.iter().map(|r| year_of(r.day)).collect::<HashSet<_>>().into_iter().collect();
    years.sort();
    let mut yt: HashMap<i64, HashMap<u32, u32>> = HashMap::new();
    let mut ya: HashMap<i64, HashMap<u32, u32>> = HashMap::new();
    let mut y_plays: HashMap<i64, u64> = HashMap::new();
    let mut y_eps: HashMap<i64, HashSet<u32>> = HashMap::new();
    let mut y_artists: HashMap<i64, HashSet<u32>> = HashMap::new();
    let mut weekday = [0u64; 7];
    let mut hour = [0u64; 24];
    for r in &rows {
        let y = year_of(r.day);
        *yt.entry(y).or_default().entry(r.ti).or_insert(0) += 1;
        if r.ai != u32::MAX {
            *ya.entry(y).or_default().entry(r.ai).or_insert(0) += 1;
            y_artists.entry(y).or_default().insert(r.ai);
        }
        *y_plays.entry(y).or_insert(0) += 1;
        y_eps.entry(y).or_default().insert(r.ep);
        weekday[(r.day + 3).rem_euclid(7) as usize] += 1; // 1970-01-01 was a Thursday
        hour[(hours[r.ep as usize] as usize).min(23)] += 1;
    }

    // ------------------------------------------------------------ windows
    let latest_year_start = Day::from_ymd(year_of(latest), 1, 1).0;
    let window_defs = [
        ("today", "The last day in the data", latest, latest),
        ("week", "The last 7 days", latest - 6, latest),
        ("month", "The last 30 days", latest - 29, latest),
        ("year", "This year so far", latest_year_start, latest),
    ];
    let mut windows = serde_json::Map::new();
    for (key, label, from, to) in window_defs {
        let (mut tc, mut ac): (HashMap<u32, u32>, HashMap<u32, u32>) = (HashMap::new(), HashMap::new());
        let mut eps = HashSet::new();
        let mut n = 0u64;
        for r in rows.iter().filter(|r| r.day >= from && r.day <= to) {
            *tc.entry(r.ti).or_insert(0) += 1;
            if r.ai != u32::MAX {
                *ac.entry(r.ai).or_insert(0) += 1;
            }
            eps.insert(r.ep);
            n += 1;
        }
        let once = tc.values().filter(|&&c| c == 1).count();
        windows.insert(
            key.to_string(),
            json!({
                "label": label, "from": iso(from), "to": iso(to),
                "plays": n, "episodes": eps.len(), "distinct_tracks": tc.len(),
                "distinct_artists": ac.len(), "played_once": once,
                "tracks": top(&tc, 100).iter().map(|&(t, c)| trow(t, c)).collect::<Vec<_>>(),
                "artists": top(&ac, 100).iter().map(|&(a, c)| arow(a, c)).collect::<Vec<_>>(),
            }),
        );
    }
    let windows = Value::Object(windows);
    write_json(&out.join("windows.json"), &windows)?;

    // --------------------------------------------------------- years.json
    let mut year_list = Vec::new();
    for &y in &years {
        year_list.push(json!({
            "year": y,
            "plays": y_plays[&y],
            "episodes": y_eps[&y].len(),
            "artists": y_artists.get(&y).map(|s| s.len()).unwrap_or(0),
            "tracks": top(&yt[&y], 100).iter().map(|&(t, c)| trow(t, c)).collect::<Vec<_>>(),
            "top_artists": top(ya.get(&y).unwrap_or(&HashMap::new()), 100).iter().map(|&(a, c)| arow(a, c)).collect::<Vec<_>>(),
        }));
    }
    write_json(&out.join("years.json"), &json!(year_list))?;

    // --------------------------------------------------------- tracks_top / rare / artists_top
    let mut order: Vec<u32> = (0..tracks.len() as u32).collect();
    order.sort_by(|&a, &b| tracks[b as usize].plays.cmp(&tracks[a as usize].plays).then(a.cmp(&b)));
    let top_tracks: Vec<Value> = order
        .iter()
        .take(3000)
        .map(|&t| {
            let mut v = trow(t, tracks[t as usize].plays);
            if let Some(a) = v.as_array_mut() {
                a.push(json!(iso(tracks[t as usize].first)));
                a.push(json!(iso(tracks[t as usize].last)));
            }
            v
        })
        .collect();
    write_json(&out.join("tracks_top.json"), &json!(top_tracks))?;

    let mut once: Vec<u32> = (0..tracks.len() as u32).filter(|&t| tracks[t as usize].plays == 1).collect();
    let once_total = once.len();
    once.sort_by(|&a, &b| tracks[b as usize].last.cmp(&tracks[a as usize].last).then(a.cmp(&b)));
    let rare: Vec<Value> = once
        .iter()
        .take(1000)
        .map(|&t| {
            let mut v = trow(t, 1);
            if let Some(a) = v.as_array_mut() {
                a.push(json!(iso(tracks[t as usize].last)));
            }
            v
        })
        .collect();
    write_json(&out.join("tracks_rare.json"), &json!({ "total": once_total, "rows": rare }))?;

    // ------------------------------------------------------ crate dive: one-off plays by year
    // A one-off track has exactly one play in the whole archive; a one-off artist has exactly one
    // play in total, so their single track is the only thing they have ever had played.
    let mut once_t: HashMap<i64, Vec<u32>> = HashMap::new();
    let mut once_a: HashMap<i64, Vec<(u32, u32)>> = HashMap::new();
    for (ti, t) in tracks.iter().enumerate() {
        if t.plays != 1 {
            continue;
        }
        let y = year_of(t.first);
        once_t.entry(y).or_default().push(ti as u32);
        if t.ai != u32::MAX && artists[t.ai as usize].plays == 1 {
            once_a.entry(y).or_default().push((t.ai, ti as u32));
        }
    }
    let mut crate_list = Vec::new();
    for &y in &years {
        let mut ts = once_t.remove(&y).unwrap_or_default();
        ts.sort_by_key(|&t| (tracks[t as usize].first, t));
        let mut asx = once_a.remove(&y).unwrap_or_default();
        asx.sort_by_key(|&(_, t)| (tracks[t as usize].first, t));
        let trows: Vec<Value> = spread(&ts, CRATE_ROWS)
            .iter()
            .map(|&t| {
                let mut v = trow(t, 1);
                if let Some(a) = v.as_array_mut() {
                    a.push(json!(iso(tracks[t as usize].first)));
                }
                v
            })
            .collect();
        let arows: Vec<Value> = spread(&asx, CRATE_ROWS)
            .iter()
            .map(|&(a, t)| {
                let ar = &artists[a as usize];
                json!([ar.name, ar.id, ar.letter, tracks[t as usize].title, iso(tracks[t as usize].first)])
            })
            .collect();
        let summary = json!({
            "year": y,
            "tracks": yt[&y].len(),
            "once_tracks": ts.len(),
            "artists": y_artists.get(&y).map(|s| s.len()).unwrap_or(0),
            "once_artists": asx.len(),
        });
        let mut detail = summary.clone();
        detail["track_rows"] = json!(trows);
        detail["artist_rows"] = json!(arows);
        write_json(&out.join("crate").join(format!("{y}.json")), &detail)?;
        crate_list.push(summary);
    }
    write_json(&out.join("crate.json"), &json!(crate_list))?;

    let atop: Vec<Value> = aorder
        .iter()
        .take(1000)
        .map(|&a| {
            let m = mb_of(&artists[a as usize]);
            let ar = &artists[a as usize];
            json!([ar.name, ar.id, ar.letter, ar.plays, ar.tracks, m.genre, m.country])
        })
        .collect();
    write_json(&out.join("artists_top.json"), &json!(atop))?;

    // ------------------------------------------------------------- shows
    let mut s_plays = vec![0u64; shows.len()];
    let mut s_tracks: Vec<HashMap<u32, u32>> = vec![HashMap::new(); shows.len()];
    let mut s_artists: Vec<HashMap<u32, u32>> = vec![HashMap::new(); shows.len()];
    let mut s_eps: Vec<HashSet<u32>> = vec![HashSet::new(); shows.len()];
    let mut s_first = vec![i64::MAX; shows.len()];
    let mut s_last = vec![i64::MIN; shows.len()];
    for r in &rows {
        let s = r.si as usize;
        s_plays[s] += 1;
        *s_tracks[s].entry(r.ti).or_insert(0) += 1;
        if r.ai != u32::MAX {
            *s_artists[s].entry(r.ai).or_insert(0) += 1;
        }
        s_eps[s].insert(r.ep);
        s_first[s] = s_first[s].min(r.day);
        s_last[s] = s_last[s].max(r.day);
    }
    // the five latest episodes of every show
    let mut recent_by_show: Vec<VecDeque<u32>> = vec![VecDeque::new(); shows.len()];
    let mut ep_order: Vec<u32> = (0..episodes.len() as u32).collect();
    ep_order.sort_by(|&a, &b| {
        let (ea, eb) = (&episodes[a as usize], &episodes[b as usize]);
        (&ea.date, &ea.time, a).cmp(&(&eb.date, &eb.time, b))
    });
    for &e in &ep_order {
        let q = &mut recent_by_show[episodes[e as usize].show as usize];
        q.push_back(e);
        if q.len() > 5 {
            q.pop_front();
        }
    }
    let ep_json = |e: u32| -> Value {
        let ep = &episodes[e as usize];
        json!({
            "date": ep.date, "time": ep.time, "title": ep.title, "pid": ep.pid,
            "tracks": ep.tracks.iter().map(|(a, t, ai)| {
                if *ai == u32::MAX { json!([a, t, "", ""]) } else { json!([a, t, artists[*ai as usize].id, artists[*ai as usize].letter]) }
            }).collect::<Vec<_>>(),
        })
    };

    let mut show_index = Vec::new();
    let mut sorder: Vec<usize> = (0..shows.len()).collect();
    sorder.sort_by(|&a, &b| s_plays[b].cmp(&s_plays[a]).then(a.cmp(&b)));
    for &s in &sorder {
        let top_artists = top(&s_artists[s], 20);
        let top_name = top_artists.first().map(|&(a, _)| artists[a as usize].name.clone()).unwrap_or_default();
        show_index.push(json!([
            shows[s].name, shows[s].slug, s_plays[s], s_eps[s].len(), s_tracks[s].len(),
            iso(s_first[s]), iso(s_last[s]), top_name
        ]));
        let mut all: Vec<(u32, u32)> = s_tracks[s].iter().map(|(t, c)| (*t, *c)).collect();
        all.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let mut recent: Vec<u32> = recent_by_show[s].iter().cloned().collect();
        recent.reverse();
        {
            let sh = &shows[s];
            let tr: Vec<Value> = all.iter().take(100).map(|&(t, c)| trow(t, c)).collect();
            let ar: Vec<Value> = top_artists.iter().map(|&(a, c)| arow(a, c)).collect();
            let names: Vec<String> = ar.iter().take(3).filter_map(|r| r[0].as_str().map(String::from)).collect();
            let mut body = format!(
                "<p class=\"tag\"><a href=\"{}\">Shows</a></p><h1>{}</h1><p class=\"lede\">{} plays across {} episodes with tracklists, from {} to {}.</p>",
                em.u("shows/"), esc(&sh.name), thou(s_plays[s]), thou(s_eps[s].len() as u64), iso(s_first[s]), iso(s_last[s])
            );
            body.push_str(&format!("<h2>Most played artists</h2>{}<h2>Most played songs</h2>{}", em.artist_table(&ar, &page_ids, 20), em.track_table(&tr, &page_ids, 100)));
            body.push_str("<h2>Latest episodes</h2>");
            for &e in &recent {
                let ev = ep_json(e);
                body.push_str(&em.episode(&ev, &format!("{} {} - {}", ev["date"].as_str().unwrap_or(""), ev["time"].as_str().unwrap_or(""), ev["title"].as_str().unwrap_or("")), &page_ids));
            }
            let desc = format!("Every song played on {} on BBC Radio 6 Music: {} plays. Most played artists include {}.", sh.name, thou(s_plays[s]), names.join(", "));
            let ld = json!({"@context": "https://schema.org", "@type": "RadioSeries", "name": sh.name, "url": format!("{}{}", origin.trim_end_matches('/'), em.u(&format!("show/{}/", sh.slug))), "description": desc, "inLanguage": "en-GB"});
            em.page(&format!("show/{}/", sh.slug), &sh.name, &desc, &body, Some(ld), false)?;
        }
        write_json(
            &out.join("shows").join(format!("{}.json", shows[s].slug)),
            &json!({
                "name": shows[s].name, "slug": shows[s].slug,
                "plays": s_plays[s], "episodes": s_eps[s].len(),
                "first": iso(s_first[s]), "last": iso(s_last[s]),
                "top_artists": top_artists.iter().map(|&(a, c)| arow(a, c)).collect::<Vec<_>>(),
                "tracks": all.iter().map(|&(t, c)| trow(t, c)).collect::<Vec<_>>(),
                "recent": recent.iter().map(|&e| ep_json(e)).collect::<Vec<_>>(),
            }),
        )?;
    }
    write_json(&out.join("shows.json"), &json!(show_index))?;

    // ------------------------------------------------------------ latest 7 days
    let mut days: Vec<Value> = Vec::new();
    for d in (latest - 6..=latest).rev() {
        let mut eps: Vec<u32> = ep_order.iter().cloned().filter(|&e| episodes[e as usize].date == iso(d)).collect();
        eps.sort_by(|&a, &b| episodes[a as usize].time.cmp(&episodes[b as usize].time));
        days.push(json!({
            "date": iso(d),
            "episodes": eps.iter().map(|&e| {
                let mut v = ep_json(e);
                v["show"] = json!(shows[episodes[e as usize].show as usize].name);
                v["slug"] = json!(shows[episodes[e as usize].show as usize].slug);
                v
            }).collect::<Vec<_>>(),
        }));
    }
    write_json(&out.join("latest.json"), &json!(days))?;

    // ------------------------------------------------------------ artist shards
    let mut by_artist: Vec<Vec<u32>> = vec![Vec::new(); artists.len()];
    for (i, t) in tracks.iter().enumerate() {
        if t.ai != u32::MAX {
            by_artist[t.ai as usize].push(i as u32);
        }
    }
    let mut shards: HashMap<String, Vec<u32>> = HashMap::new();
    for &a in &aorder {
        shards.entry(artists[a as usize].letter.clone()).or_default().push(a);
    }
    let mut known_genre_plays = 0u64;
    let mut genre_buckets: HashMap<&'static str, u64> = HashMap::new();
    let mut raw_genres: HashMap<String, u64> = HashMap::new();
    let mut countries: HashMap<String, u64> = HashMap::new();
    for a in &artists {
        let m = mb_of(a);
        if !m.genre.is_empty() {
            known_genre_plays += a.plays as u64;
            *genre_buckets.entry(classify(&m.genre)).or_insert(0) += a.plays as u64;
            *raw_genres.entry(m.genre.clone()).or_insert(0) += a.plays as u64;
        }
        if !m.country.is_empty() {
            *countries.entry(m.country).or_insert(0) += a.plays as u64;
        }
    }
    for (letter, list) in &shards {
        let rows: Vec<Value> = list
            .iter()
            .map(|&a| {
                let ar = &artists[a as usize];
                let m = mb_of(ar);
                let mut ts = by_artist[a as usize].clone();
                ts.sort_by(|&x, &y| tracks[y as usize].plays.cmp(&tracks[x as usize].plays).then(x.cmp(&y)));
                json!({
                    "id": ar.id, "name": ar.name, "mbid": ar.mbid, "plays": ar.plays, "tracks": ar.tracks,
                    "first": iso(ar.first), "last": iso(ar.last),
                    "genre": m.genre, "genres": m.genres, "country": m.country, "begin": m.begin, "type": m.kind,
                    "top": ts.iter().take(15).map(|&t| json!([tracks[t as usize].title, tracks[t as usize].plays])).collect::<Vec<_>>(),
                })
            })
            .collect();
        for &a in list.iter() {
            let ar = &artists[a as usize];
            if !page_ids.contains(&ar.id) {
                continue;
            }
            let m = mb_of(ar);
            let mut ts = by_artist[a as usize].clone();
            ts.sort_by(|&x, &y| tracks[y as usize].plays.cmp(&tracks[x as usize].plays).then(x.cmp(&y)));
            let tops: Vec<(&str, u32)> = ts.iter().take(15).map(|&t| (tracks[t as usize].title.as_str(), tracks[t as usize].plays)).collect();
            let mut facts = format!("<dt>Plays</dt><dd>{} plays of {} different tracks</dd><dt>First / last played</dt><dd>{} / {}</dd>", thou(ar.plays as u64), thou(ar.tracks as u64), iso(ar.first), iso(ar.last));
            if ar.mbid.is_empty() {
                facts.push_str("<dt>MusicBrainz ID</dt><dd>Not supplied by the BBC for this artist</dd>");
            } else {
                facts.push_str(&format!("<dt>MusicBrainz ID</dt><dd><code>{0}</code> <a href=\"https://musicbrainz.org/artist/{0}\" rel=\"noopener\">View on MusicBrainz</a></dd>", esc(&ar.mbid)));
            }
            for (k, v) in [("Type", &m.kind), ("Country", &m.country), ("Began", &m.begin), ("Genres (MusicBrainz tags)", &m.genres)] {
                if !v.is_empty() {
                    facts.push_str(&format!("<dt>{k}</dt><dd>{}</dd>", esc(&v.replace(';', ", "))));
                }
            }
            let mut body = format!(
                "<p class=\"tag\"><a href=\"{}\">Artists</a> / <a href=\"{}\">{}</a></p><h1>{}</h1><dl class=\"facts\">{}</dl>",
                em.u("artists/"), em.u(&format!("artists/{letter}/")), esc(&letter.to_uppercase()), esc(&ar.name), facts
            );
            body.push_str("<h2>Most played tracks</h2><div class=\"tablewrap\"><table><thead><tr><th>Track</th><th class=\"n\">Plays</th><th>Listen</th></tr></thead><tbody>");
            for (title, plays) in &tops {
                body.push_str(&format!("<tr><td>{}</td><td class=\"n\">{}</td><td class=\"listen\">{}</td></tr>", esc(title), plays, listen_links(&ar.name, title)));
            }
            body.push_str("</tbody></table></div><p class=\"note\">YT, SP, SC and BC open a search for the track on YouTube, Spotify, SoundCloud and Bandcamp. They are ordinary links: nothing from those sites is loaded here.</p>");
            let top3: Vec<&str> = tops.iter().take(3).map(|t| t.0).collect();
            let desc = format!("{} on BBC Radio 6 Music: {} plays of {} tracks between {} and {}. Most played: {}.", ar.name, thou(ar.plays as u64), thou(ar.tracks as u64), iso(ar.first), iso(ar.last), top3.join(", "));
            let url = format!("{}{}", origin.trim_end_matches('/'), em.u(&format!("artist/{letter}/{}/", ar.id)));
            let mut ld = json!({"@context": "https://schema.org", "@type": if m.kind == "Person" { "Person" } else { "MusicGroup" }, "name": ar.name, "url": url});
            if !ar.mbid.is_empty() {
                ld["sameAs"] = json!([format!("https://musicbrainz.org/artist/{}", ar.mbid)]);
            }
            em.page(&format!("artist/{letter}/{}/", ar.id), &ar.name, &desc, &body, Some(ld), false)?;
        }
        write_json(&out.join("artists").join(format!("{letter}.json")), &json!(rows))?;
    }

    // ------------------------------------------------------------ summary + meta
    let mut gb: Vec<(&str, u64)> = genre_buckets.into_iter().collect();
    gb.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    let mut rg: Vec<(String, u64)> = raw_genres.into_iter().collect();
    rg.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut cs: Vec<(String, u64)> = countries.into_iter().collect();
    cs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let total_plays = rows.len() as u64;
    let with_mbid_plays = rows.iter().filter(|r| r.ai != u32::MAX && !artists[r.ai as usize].mbid.is_empty()).count() as u64;
    write_json(
        &out.join("summary.json"),
        &json!({
            "by_year": years.iter().map(|y| json!([y, y_plays[y], y_eps[y].len(), y_artists.get(y).map(|s| s.len()).unwrap_or(0)])).collect::<Vec<_>>(),
            "weekday": weekday, "hour": hour,
            "top_shows": sorder.iter().take(25).map(|&s| json!([shows[s].name, shows[s].slug, s_plays[s]])).collect::<Vec<_>>(),
            "genre_buckets": gb.iter().map(|(n, c)| json!([n, c])).collect::<Vec<_>>(),
            "raw_genres": rg.iter().take(25).map(|(n, c)| json!([n, c])).collect::<Vec<_>>(),
            "countries": cs.iter().take(20).map(|(n, c)| json!([n, c])).collect::<Vec<_>>(),
            "genre_coverage_pct": (known_genre_plays as f64 * 1000.0 / total_plays as f64).round() / 10.0,
            "mbid_coverage_pct": (with_mbid_plays as f64 * 1000.0 / total_plays as f64).round() / 10.0,
        }),
    )?;
    write_json(
        &out.join("meta.json"),
        &json!({
            "as_of": iso(latest), "first_day": iso(earliest),
            "plays": total_plays, "episodes": episodes.len(), "tracks": tracks.len(),
            "artists": artists.len(), "shows": shows.len(),
            "artists_with_mbid": artists.iter().filter(|a| !a.mbid.is_empty()).count(),
            "artists_with_genre": artists.iter().filter(|a| !mb_of(a).genre.is_empty()).count(),
            "one_play_tracks": once_total,
        }),
    )?;
    // ------------------------------------------------------------ static pages
    let o = origin.trim_end_matches('/').to_string();
    let home_url = format!("{}{}", o, em.u(""));
    let names_of = |rows: &[Value], n: usize| -> String {
        rows.iter().take(n).map(|r| format!("{} - {}", r[0].as_str().unwrap_or(""), r[1].as_str().unwrap_or(""))).collect::<Vec<_>>().join("; ")
    };
    let arr = |v: &Value| v.as_array().cloned().unwrap_or_default();

    // home
    {
        let wk = arr(&windows["week"]["tracks"]);
        let body = format!(
            "<h1>Every track played on BBC Radio 6 Music.</h1><p class=\"lede\">An archive of {} plays across {} shows, from {} to {}. Updated every day.</p>\
             <ul><li><a href=\"{}\">Most played: today, this week, this month, this year, all time</a></li><li><a href=\"{}\">Latest: every tracklist from the last 7 days</a></li>\
             <li><a href=\"{}\">Shows: every song each show has played</a></li><li><a href=\"{}\">Artists A to Z, with MusicBrainz IDs</a></li><li><a href=\"{}\">Years</a></li><li><a href=\"{}\">Crate dive: tracks and artists played only once</a></li><li><a href=\"{}\">Downloads (CSV)</a></li></ul>\
             <h2>Most played this week</h2>{}",
            thou(rows.len() as u64), thou(shows.len() as u64), iso(earliest), iso(latest),
            em.u("most-played/"), em.u("latest/"), em.u("shows/"), em.u("artists/"), em.u("years/"), em.u("crate-dive/"), em.u("downloads/"),
            em.track_table(&wk, &page_ids, 20)
        );
        let desc = "An unofficial, non-commercial archive of every track played on BBC Radio 6 Music: shows, artists, charts and downloads. Updated daily.";
        let ld = json!({
            "@context": "https://schema.org", "@type": "Dataset",
            "name": "Every track played on BBC Radio 6 Music", "description": desc,
            "url": home_url, "isAccessibleForFree": true, "inLanguage": "en-GB",
            "temporalCoverage": format!("{}/{}", iso(earliest), iso(latest)),
            "keywords": ["BBC Radio 6 Music", "playlist", "tracklist", "radio", "music"],
            "distribution": [{"@type": "DataDownload", "encodingFormat": "text/csv", "contentUrl": format!("{REPO}/releases/tag/data-latest")}],
            "sameAs": [REPO]
        });
        em.page("", "", desc, &body, Some(ld), false)?;
    }

    // most played
    {
        let mut hub = String::from("<h1>Most played on BBC Radio 6 Music</h1><p class=\"lede\">The most played tracks and artists for different periods.</p><ul>");
        for (k, label) in [("today", "The last day in the data"), ("week", "The last 7 days"), ("month", "The last 30 days"), ("year", "This year so far"), ("all", "All time")] {
            hub.push_str(&format!("<li><a href=\"{}\">{}</a></li>", em.u(&format!("most-played/{k}/")), label));
        }
        hub.push_str(&format!("<li><a href=\"{}\">Least played: the crate dive, tracks and artists played only once, year by year</a></li>", em.u("crate-dive/")));
        hub.push_str("</ul>");
        em.page("most-played/", "Most played", "The most played tracks and artists on BBC Radio 6 Music: last day, week, month, year, all time, and the least played.", &hub, None, false)?;
        for (k, label) in [("today", "the last day in the data"), ("week", "the last 7 days"), ("month", "the last 30 days"), ("year", "this year so far")] {
            let w = &windows[k];
            let (tr, ar) = (arr(&w["tracks"]), arr(&w["artists"]));
            let body = format!(
                "<h1>Most played, {label}</h1><p class=\"lede\">{} to {} · {} plays · {} different tracks.</p><h2>Tracks</h2>{}<h2>Artists</h2>{}",
                w["from"].as_str().unwrap_or(""), w["to"].as_str().unwrap_or(""), thou(w["plays"].as_u64().unwrap_or(0)), thou(w["distinct_tracks"].as_u64().unwrap_or(0)),
                em.track_table(&tr, &page_ids, 100), em.artist_table(&ar, &page_ids, 100)
            );
            let desc = format!("The most played tracks and artists on BBC Radio 6 Music, {label}. Top tracks: {}.", names_of(&tr, 3));
            em.page(&format!("most-played/{k}/"), &format!("Most played tracks and artists, {label}"), &desc, &body, None, false)?;
        }
        let (tt, aa) = (&top_tracks, &atop);
        let body = format!(
            "<h1>Most played, all time</h1><p class=\"lede\">{} to {} · {} plays · {} different tracks · {} of them played only once</p><p class=\"note\">Some of the most-played entries are programme theme tunes and jingles, which the BBC logs as tracks.</p><h2>Tracks</h2>{}<h2>Artists</h2>{}",
            iso(earliest), iso(latest), thou(rows.len() as u64), thou(tracks.len() as u64), thou(once_total as u64),
            em.track_table(tt, &page_ids, 200), em.artist_table(aa, &page_ids, 200)
        );
        let desc = format!("The most played tracks and artists on BBC Radio 6 Music of all time. Top tracks: {}.", names_of(tt, 3));
        em.page("most-played/all/", "Most played tracks and artists of all time", &desc, &body, None, false)?;
    }

    // latest
    {
        let mut body = String::from("<h1>Played in the last 7 days</h1><p class=\"lede\">Every episode with a tracklist, newest first.</p>");
        for d in &days {
            body.push_str(&format!("<h2>{}</h2>", d["date"].as_str().unwrap_or("")));
            for e in arr(&d["episodes"]) {
                let head = format!("{} {} - {}", e["time"].as_str().unwrap_or(""), e["show"].as_str().unwrap_or(""), e["title"].as_str().unwrap_or(""));
                body.push_str(&em.episode(&e, &head, &page_ids));
            }
        }
        em.page("latest/", "Latest tracklists", "Every BBC Radio 6 Music episode with its tracklist from the last 7 days, updated daily.", &body, None, false)?;
    }

    // shows
    {
        let mut body = format!("<h1>Shows</h1><p class=\"lede\">{} shows. Open one to see every song it has played.</p><div class=\"tablewrap\"><table><thead><tr><th>Show</th><th class=\"n\">Plays</th><th class=\"n\">Episodes</th><th>First</th><th>Last</th></tr></thead><tbody>", thou(shows.len() as u64));
        for r in &show_index {
            body.push_str(&format!(
                "<tr><td><a href=\"{}\">{}</a></td><td class=\"n\">{}</td><td class=\"n\">{}</td><td>{}</td><td>{}</td></tr>",
                em.u(&format!("show/{}/", r[1].as_str().unwrap_or(""))), esc(r[0].as_str().unwrap_or("")), r[2], r[3], r[5].as_str().unwrap_or(""), r[6].as_str().unwrap_or("")
            ));
        }
        body.push_str("</tbody></table></div>");
        em.page("shows/", "Shows", "Every BBC Radio 6 Music show with every song it has played, from the archive of tracklists.", &body, None, false)?;
    }

    // artists: top, and each letter
    {
        let nav: String = std::iter::once(("".to_string(), "Top".to_string()))
            .chain("0abcdefghijklmnopqrstuvwxyz".chars().chain(std::iter::once('~')).map(|c| match c {
                '0' => ("0/".to_string(), "0-9".to_string()),
                '~' => ("other/".to_string(), "other".to_string()),
                c => (format!("{c}/"), c.to_string()),
            }))
            .map(|(p, l)| format!("<a href=\"{}\">{}</a>", em.u(&format!("artists/{p}")), l))
            .collect::<Vec<_>>()
            .join(" ");
        let body = format!("<h1>Artists</h1><p class=\"lede\">Each artist has a MusicBrainz ID where the BBC supplied one. Choose a letter to browse everyone.</p><nav class=\"az\" aria-label=\"Artists A to Z\">{nav}</nav><h2>Most played</h2>{}", em.artist_table(&atop, &page_ids, 1000));
        em.page("artists/", "Artists", "BBC Radio 6 Music artists A to Z, with play counts and MusicBrainz IDs. The 1,000 most played are listed here.", &body, None, false)?;
        for (letter, list) in &shards {
            let rowsv: Vec<Value> = list.iter().take(300).map(|&a| arow(a, artists[a as usize].plays)).collect();
            let label = match letter.as_str() { "0" => "0-9".to_string(), "other" => "other characters".to_string(), l => l.to_uppercase() };
            let body = format!("<h1>Artists: {label}</h1><p class=\"lede\">{} artists. The 300 most played are listed here; the rest are searchable.</p><nav class=\"az\">{nav}</nav>{}", thou(list.len() as u64), em.artist_table(&rowsv, &page_ids, 300));
            em.page(&format!("artists/{letter}/"), &format!("Artists {label}"), &format!("BBC Radio 6 Music artists starting with {label}: play counts and MusicBrainz IDs."), &body, None, false)?;
        }
    }

    // years
    {
        let mut body = String::from("<h1>Years</h1><p class=\"lede\">Tracklists begin in mid-2008 and are complete from about 2010.</p><ul>");
        for y in year_list.iter().rev() {
            body.push_str(&format!("<li><a href=\"{}\">{}</a> · {} plays</li>", em.u(&format!("year/{}/", y["year"])), y["year"], thou(y["plays"].as_u64().unwrap_or(0))));
        }
        body.push_str("</ul>");
        em.page("years/", "Years", "BBC Radio 6 Music plays per year, with a top 100 tracks and artists for every year since 2008.", &body, None, false)?;
        for y in &year_list {
            let (tr, ar) = (arr(&y["tracks"]), arr(&y["top_artists"]));
            let body = format!(
                "<p class=\"tag\"><a href=\"{}\">Years</a></p><h1>{}</h1><p class=\"lede\">{} plays · {} episodes · {} different artists.</p><h2>Most played tracks</h2>{}<h2>Most played artists</h2>{}",
                em.u("years/"), y["year"], thou(y["plays"].as_u64().unwrap_or(0)), thou(y["episodes"].as_u64().unwrap_or(0)), thou(y["artists"].as_u64().unwrap_or(0)),
                em.track_table(&tr, &page_ids, 100), em.artist_table(&ar, &page_ids, 100)
            );
            let desc = format!("The most played tracks and artists on BBC Radio 6 Music in {}. Top tracks: {}.", y["year"], names_of(&tr, 3));
            em.page(&format!("year/{}/", y["year"]), &format!("Most played of {}", y["year"]), &desc, &body, None, false)?;
        }
    }

    // crate dive: tracks and artists played only once, year by year
    {
        let mut body = format!(
            "<h1>Crate dive</h1><p class=\"lede\">{} tracks in the archive have been played exactly once. Here they are, year by year: the deepest cuts to reach 6 Music.</p>\
             <p class=\"note\">A one-off is a track, or an artist, with a single play in the whole archive. Recent years score higher because new tracks have had less time to be played again, and tracklists before about 2010 are incomplete.</p>\
             <div class=\"tablewrap\"><table><thead><tr><th>Year</th><th class=\"n\">One-off tracks</th><th class=\"n\">Of all tracks</th><th class=\"n\">One-off artists</th><th class=\"n\">Of all artists</th></tr></thead><tbody>",
            thou(once_total as u64)
        );
        let pct = |a: u64, b: u64| if b == 0 { "0%".to_string() } else { format!("{:.0}%", a as f64 * 100.0 / b as f64) };
        for y in crate_list.iter().rev() {
            let (t, ot, a, oa) = (y["tracks"].as_u64().unwrap_or(0), y["once_tracks"].as_u64().unwrap_or(0), y["artists"].as_u64().unwrap_or(0), y["once_artists"].as_u64().unwrap_or(0));
            body.push_str(&format!(
                "<tr><td><a href=\"{}\">{}</a></td><td class=\"n\">{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td></tr>",
                em.u(&format!("crate-dive/{}/", y["year"])), y["year"], thou(ot), pct(ot, t), thou(oa), pct(oa, a)
            ));
        }
        body.push_str("</tbody></table></div>");
        em.page("crate-dive/", "Crate dive: tracks and artists played only once", "Tracks and artists played only once on BBC Radio 6 Music, counted year by year since 2008: the deepest cuts the station has ever played.", &body, None, false)?;
        for y in &crate_list {
            let detail: Value = serde_json::from_reader(std::io::BufReader::new(fs::File::open(out.join("crate").join(format!("{}.json", y["year"]))).map_err(err("cannot read crate file"))?)).map_err(err("json"))?;
            let mut body = format!(
                "<p class=\"tag\"><a href=\"{}\">Crate dive</a></p><h1>{}</h1><p class=\"lede\">{} tracks and {} artists played only once, ever. {} different tracks were played in {}.</p><h2>Tracks</h2><div class=\"tablewrap\"><table><thead><tr><th>Artist</th><th>Track</th><th>Played on</th></tr></thead><tbody>",
                em.u("crate-dive/"), y["year"], thou(y["once_tracks"].as_u64().unwrap_or(0)), thou(y["once_artists"].as_u64().unwrap_or(0)), thou(y["tracks"].as_u64().unwrap_or(0)), y["year"]
            );
            for r in arr(&detail["track_rows"]).iter().take(100) {
                body.push_str(&format!("<tr><td>{}</td><td>{}</td><td>{}</td></tr>", em.artist_link(r[0].as_str().unwrap_or(""), r[3].as_str().unwrap_or(""), r[4].as_str().unwrap_or(""), &page_ids), esc(r[1].as_str().unwrap_or("")), r[5].as_str().unwrap_or("")));
            }
            body.push_str("</tbody></table></div><h2>Artists</h2><div class=\"tablewrap\"><table><thead><tr><th>Artist</th><th>Track</th><th>Played on</th></tr></thead><tbody>");
            for r in arr(&detail["artist_rows"]).iter().take(100) {
                body.push_str(&format!("<tr><td>{}</td><td>{}</td><td>{}</td></tr>", em.artist_link(r[0].as_str().unwrap_or(""), r[1].as_str().unwrap_or(""), r[2].as_str().unwrap_or(""), &page_ids), esc(r[3].as_str().unwrap_or("")), r[4].as_str().unwrap_or("")));
            }
            body.push_str("</tbody></table></div>");
            let desc = format!("Tracks and artists played only once on BBC Radio 6 Music in {}: {} one-off tracks and {} one-off artists.", y["year"], y["once_tracks"], y["once_artists"]);
            em.page(&format!("crate-dive/{}/", y["year"]), &format!("Crate dive {}: played only once", y["year"]), &desc, &body, None, false)?;
        }
    }

    // downloads and about
    em.page(
        "downloads/", "Downloads",
        "Download the BBC Radio 6 Music play-by-play dataset as CSV: tracks, artists, shows and episodes. Free for personal, non-commercial use.",
        &format!("<h1>Downloads</h1><p class=\"lede\">The whole dataset is plain CSV. See the <a href=\"{REPO}/tree/main/data\">data folder</a> and the <a href=\"{REPO}/releases/tag/data-latest\">latest daily build</a>. Every table on this site also has CSV and TXT buttons.</p>"),
        None, false,
    )?;
    let about = fs::read_to_string(src.join("about.html")).map_err(err("cannot read about.html"))?;
    em.page("about/", "About", "How this unofficial BBC Radio 6 Music archive is made, kept up to date and collected politely, and what its data can and cannot tell you.", &about, None, true)?;

    let pages_written = em.count() + 1;
    em.finish()?;
    println!("{pages_written} pages, sitemap.xml, robots.txt and 404.html written to {}", root.display());
    println!("Site data written to {}", out.display());
    Ok(())
}

/// At most `cap` items spread evenly through `v`, so a long list is not just its first days.
fn spread<T: Clone>(v: &[T], cap: usize) -> Vec<T> {
    if v.len() <= cap {
        return v.to_vec();
    }
    (0..cap).map(|i| v[i * v.len() / cap].clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_ignore_the_and_case() {
        assert_eq!(letter_of("The Fall"), "f");
        assert_eq!(letter_of("  ABBA"), "a");
        assert_eq!(letter_of("808 State"), "0");
        assert_eq!(letter_of("!!!"), "other");
    }

    #[test]
    fn slugs_are_url_safe_and_unique() {
        assert_eq!(slug("Steve Lamacq's Roundtable"), "steve-lamacq-s-roundtable");
        assert_eq!(slug("???"), "");
        let mut used = HashSet::new();
        assert_eq!(unique("a".into(), &mut used), "a");
        assert_eq!(unique("a".into(), &mut used), "a-2");
        assert_eq!(unique(String::new(), &mut used), "x");
    }

    #[test]
    fn spread_keeps_everything_when_short_and_samples_evenly_when_long() {
        assert_eq!(spread(&[1, 2, 3], 5), vec![1, 2, 3]);
        let v: Vec<u32> = (0..10).collect();
        assert_eq!(spread(&v, 5), vec![0, 2, 4, 6, 8]);
    }

    #[test]
    fn thousands_separators() {
        assert_eq!(thou(0), "0");
        assert_eq!(thou(999), "999");
        assert_eq!(thou(1234567), "1,234,567");
    }

    #[test]
    fn weekdays_start_on_thursday_1970() {
        assert_eq!((0i64 + 3).rem_euclid(7), 3); // Monday = 0, so Thursday = 3
    }
}
