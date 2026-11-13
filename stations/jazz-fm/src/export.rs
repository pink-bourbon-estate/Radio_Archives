//! Write the plays in the format the shared site generator (`bbc6 site`) reads, so Jazz FM gets
//! the same pages and charts as the 6 Music site, plus one plain `plays.csv` for downloading.
//!
//! The playlist data has no episodes, so each show slot on a day is one "episode": the show's
//! start time comes from the weekly timetable in `show.rs`.

use crate::extract::{fmt_utc, parse_time, utc_to_london};
use crate::show::{show_for, slot_start_minute};
use catalogue::Catalogue;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

fn q(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

const HEADER: &str = "Date,Time,Show,Episode,EpisodePID,Position,OffsetSec,Artist,Title,Album,Label,Publisher,DurationSec,MusicBrainzArtistID\n";

pub fn write(plays: &[(String, String, String)], cat: &Catalogue, out: &Path) -> io::Result<usize> {
    let dir = out.join("plays");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir)?;
    // (artist, title, artist's MusicBrainz ID or "")
    let name_of = |tid: &str| -> Option<(String, String, String)> {
        cat.tracks.get(tid).map(|t| {
            let mbid = cat.artists.get(&t.artist_id).map(|a| a.mbid.clone()).unwrap_or_default();
            (cat.artist_name(&t.artist_id).to_string(), t.title.clone(), mbid)
        })
    };

    let mut by_year: std::collections::BTreeMap<String, String> = Default::default();
    let mut position: HashMap<String, u32> = HashMap::new();
    let mut plain = String::from("played_at_utc,show,artist,title,artist_musicbrainz_id\n");
    let mut n = 0;
    let mut sorted: Vec<&(String, String, String)> = plays.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    for (at, tid, show) in sorted {
        let Some((artist, title, mbid)) = name_of(tid) else { continue };
        let Some((utc, _)) = parse_time(at) else { continue };
        let local = utc_to_london(utc);
        let show = if show.is_empty() { show_for(local).to_string() } else { show.clone() };
        let stamp = fmt_utc(local); // 2026-11-13T07:45:00Z, as UK wall-clock time
        let date = &stamp[..10];
        let slot = slot_start_minute(local);
        let time = format!("{:02}:{:02}", slot / 60, slot % 60);
        let key = format!("{date}|{time}|{show}");
        let pos = position.entry(key).or_insert(0);
        *pos += 1;
        let offset = local.rem_euclid(86_400) - slot * 60;
        let episode = format!("{}/{}/{}", &date[8..10], &date[5..7], &date[..4]);
        let row = format!(
            "{date},{time},{},{episode},,{pos},{},{},{},,,,,{}\n",
            q(&show),
            offset.max(0),
            q(&artist),
            q(&title),
            q(&mbid)
        );
        by_year.entry(date[..4].to_string()).or_insert_with(|| HEADER.to_string()).push_str(&row);
        plain.push_str(&format!("{at},{},{},{},{}\n", q(&show), q(&artist), q(&title), q(&mbid)));
        n += 1;
    }
    for (year, text) in by_year {
        fs::write(dir.join(format!("plays_{year}.csv")), text)?;
    }
    fs::write(out.join("plays.csv"), plain)?;
    Ok(n)
}
