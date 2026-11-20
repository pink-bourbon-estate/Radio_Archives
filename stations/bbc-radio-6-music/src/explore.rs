//! Data for the zoom control and click-to-filter dashboards.
//!
//! * `daily.json`       plays per day, so any chart can be drawn by day, week, month, quarter or year
//! * `show_daily.json`  plays per day for each show (sparse), for the show pages
//! * `month/YYYY-MM.json` every play in that month, as small integers into a per-month table of
//!   artists, tracks and shows, so the browser can total up any day, week, month, quarter or
//!   year by fetching only the months it needs.

use crate::util::*;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

/// One play: day number, track, artist (u32::MAX when blank), show, hour of day.
pub struct Play {
    pub day: i64,
    pub track: u32,
    pub artist: u32,
    pub show: u32,
    pub hour: u8,
}

fn write(path: &Path, v: &Value) -> Res<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(err("cannot create folder"))?;
    }
    let f = fs::File::create(path).map_err(err(&format!("cannot write {}", path.display())))?;
    serde_json::to_writer(std::io::BufWriter::new(f), v).map_err(err("json"))
}

/// `tracks`: (artist, title, artist index); `artists`: (name, id, letter); `shows`: (name, slug).
pub fn write_all(
    out: &Path,
    plays: &[Play],
    tracks: &[(String, String, u32)],
    artists: &[(String, String, String)],
    shows: &[(String, String)],
) -> Res<()> {
    let first = plays.iter().map(|p| p.day).min().ok_or("no plays")?;
    let last = plays.iter().map(|p| p.day).max().ok_or("no plays")?;
    let n = (last - first + 1) as usize;

    let mut daily = vec![0u32; n];
    let mut show_daily: HashMap<u32, BTreeMap<i64, u32>> = HashMap::new();
    let mut months: BTreeMap<(i64, i64), Vec<&Play>> = BTreeMap::new();
    for p in plays {
        daily[(p.day - first) as usize] += 1;
        *show_daily.entry(p.show).or_default().entry(p.day - first).or_insert(0) += 1;
        let (y, m, _) = Day(p.day).ymd();
        months.entry((y, m)).or_default().push(p);
    }
    write(&out.join("daily.json"), &json!({ "start": Day(first).iso(), "d": daily }))?;

    let mut sd = serde_json::Map::new();
    for (si, days) in &show_daily {
        let flat: Vec<i64> = days.iter().flat_map(|(d, c)| [*d, *c as i64]).collect();
        sd.insert(shows[*si as usize].1.clone(), json!(flat));
    }
    write(&out.join("show_daily.json"), &json!({ "start": Day(first).iso(), "shows": sd }))?;

    for ((y, m), list) in &months {
        let (mut ai, mut ti, mut si) = (HashMap::new(), HashMap::new(), HashMap::new());
        let (mut a_tab, mut t_tab, mut s_tab): (Vec<Value>, Vec<Value>, Vec<Value>) = (vec![], vec![], vec![]);
        let mut flat: Vec<i64> = Vec::with_capacity(list.len() * 4);
        for p in list {
            let a_local = if p.artist == u32::MAX {
                -1
            } else {
                *ai.entry(p.artist).or_insert_with(|| {
                    let a = &artists[p.artist as usize];
                    a_tab.push(json!([a.0, a.1, a.2]));
                    (a_tab.len() - 1) as i64
                })
            };
            let t_local = *ti.entry(p.track).or_insert_with(|| {
                t_tab.push(json!([a_local, tracks[p.track as usize].1]));
                (t_tab.len() - 1) as i64
            });
            let s_local = *si.entry(p.show).or_insert_with(|| {
                let s = &shows[p.show as usize];
                s_tab.push(json!([s.0, s.1]));
                (s_tab.len() - 1) as i64
            });
            flat.extend([Day(p.day).ymd().2, t_local, s_local, p.hour as i64]);
        }
        write(&out.join("month").join(format!("{y:04}-{m:02}.json")), &json!({ "a": a_tab, "t": t_tab, "s": s_tab, "p": flat }))?;
    }
    println!("zoom data: {} days, {} months", n, months.len());
    Ok(())
}
