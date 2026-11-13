//! Add genre / country / era to artists from the MusicBrainz web service.
//!
//! MusicBrainz is a volunteer-run non-profit: one request per second at most,
//! and a User-Agent with a contact. Both are enforced. Artists are looked up
//! most-played first, responses are cached, and artists already present in
//! artists_enriched.csv are skipped, so this can be stopped and resumed freely.

use crate::util::*;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const MB: &str = "https://musicbrainz.org/ws/2";
const HEADER: [&str; 15] = [
    "Artist",
    "MusicBrainzArtistID",
    "PrimaryGenre",
    "Genres",
    "Tags",
    "Country",
    "Area",
    "BeginArea",
    "Type",
    "BeginYear",
    "EndYear",
    "Decade",
    "Disambiguation",
    "DistinctTracks",
    "TotalPlays",
];

fn named_list(v: &Value, key: &str) -> Vec<(String, i64)> {
    v.get(key)
        .and_then(|l| l.as_array())
        .map(|l| {
            l.iter()
                .filter_map(|i| {
                    Some((
                        i.get("name")?.as_str()?.to_string(),
                        i.get("count").and_then(|c| c.as_i64()).unwrap_or(0),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn s(v: &Value, k: &str) -> String {
    scrub_emails(v.get(k).and_then(|x| x.as_str()).unwrap_or("")).into_owned()
}

/// One MusicBrainz artist response -> the columns of artists_enriched.csv (first 13).
pub fn extract(d: &Value) -> Option<Vec<String>> {
    if !d.is_object() {
        return None;
    }
    let tags = named_list(d, "tags");
    let mut genres = named_list(d, "genres");
    if genres.is_empty() {
        genres = tags.clone();
    }
    genres.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let life = d.get("life-span").cloned().unwrap_or(Value::Null);
    let year = |k: &str| s(&life, k).chars().take(4).collect::<String>();
    let (begin, end) = (year("begin"), year("end"));
    let decade = begin
        .parse::<i64>()
        .map(|y| format!("{}s", y / 10 * 10))
        .unwrap_or_default();
    let join = |l: &[(String, i64)]| {
        l.iter()
            .take(8)
            .map(|x| x.0.as_str())
            .collect::<Vec<_>>()
            .join("; ")
    };
    let area = |k: &str| d.get(k).map(|a| s(a, "name")).unwrap_or_default();
    Some(vec![
        genres.first().map(|g| g.0.clone()).unwrap_or_default(),
        join(&genres),
        join(&tags),
        s(d, "country"),
        area("area"),
        area("begin-area"),
        s(d, "type"),
        begin,
        end,
        decade,
        s(d, "disambiguation"),
    ])
}

fn fetch(
    agent: &ureq::Agent,
    cache: &Path,
    mbid: &str,
    gap: Duration,
    last: &mut Instant,
    offline: bool,
) -> Option<Value> {
    let path = cache.join(format!("{mbid}.json"));
    if let Ok(t) = fs::read_to_string(&path) {
        return if t == "__missing__" {
            None
        } else {
            serde_json::from_str(&t).ok()
        };
    }
    if offline {
        return None;
    }
    for attempt in 0..4u32 {
        let since = last.elapsed();
        if since < gap {
            std::thread::sleep(gap - since);
        }
        *last = Instant::now();
        match agent
            .get(&format!("{MB}/artist/{mbid}"))
            .query("inc", "genres+tags")
            .query("fmt", "json")
            .call()
        {
            Ok(resp) => {
                let mut body = String::new();
                resp.into_reader()
                    .take(16 << 20)
                    .read_to_string(&mut body)
                    .ok()?;
                let v: Value = serde_json::from_str(&body).ok()?;
                let _ = fs::write(&path, &body);
                return Some(v);
            }
            Err(ureq::Error::Status(404, _)) => {
                let _ = fs::write(&path, "__missing__");
                return None;
            }
            Err(ureq::Error::Status(code, _))
                if attempt < 3 && matches!(code, 429 | 500 | 502 | 503 | 504) => {}
            Err(ureq::Error::Transport(_)) if attempt < 3 => {}
            Err(e) => {
                eprintln!("[warn] {mbid}: {e}");
                return None;
            }
        }
        std::thread::sleep(Duration::from_secs(2 << attempt));
    }
    None
}

pub fn run(args: &Args) -> Res<()> {
    let data = PathBuf::from(args.str_or("data", "data"));
    let contact = args.str_or("contact", "");
    let offline = args.flag("offline");
    let min_plays: u64 = args.num("min-plays", 1u64)?;
    let limit: usize = args.num("limit", 0usize)?;
    let gap = Duration::from_secs_f64(args.num("gap", 1.1f64)?.max(1.0)); // MusicBrainz's published limit
    if contact.is_empty() && !offline {
        return Err("please pass --contact you@example.com (or a URL). MusicBrainz asks for a contact in the User-Agent; it is sent nowhere else.".into());
    }
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(30))
        .user_agent(&format!("bbc6/{} ( {contact} )", env!("CARGO_PKG_VERSION")))
        .build();
    let cache = data.join("_mbcache");
    fs::create_dir_all(&cache).map_err(err("cannot create cache folder"))?;

    // artists to look up
    let unique = data.join("artists_unique.csv");
    let mut r = open_csv(&unique).map_err(|e| format!("{e} (run `bbc6 derive` first)"))?;
    let c = Cols::of(&mut r, &unique)?;
    let mut todo: Vec<(String, String, String, u64)> = Vec::new(); // name, mbid, distinct tracks, plays
    for rec in r.records().flatten() {
        let mbid = c.get(&rec, "MusicBrainzArtistID").trim().to_string();
        let plays = parse_plays(c.get(&rec, "Total plays"));
        if !mbid.is_empty() && plays >= min_plays {
            todo.push((
                c.get(&rec, "Artist").to_string(),
                mbid,
                c.get(&rec, "Distinct tracks").to_string(),
                plays,
            ));
        }
    }
    todo.sort_by_key(|t| std::cmp::Reverse(t.3));

    // existing results are kept
    let out = data.join("artists_enriched.csv");
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut have: HashMap<String, usize> = HashMap::new();
    if out.exists() {
        let mut r = open_csv(&out)?;
        let c = Cols::of(&mut r, &out)?;
        for rec in r.records().flatten() {
            let row: Vec<String> = HEADER
                .iter()
                .map(|h| scrub_emails(c.get(&rec, h)).into_owned())
                .collect();
            have.insert(row[1].clone(), rows.len());
            rows.push(row);
        }
    }
    let refresh = args.flag("refresh");
    todo.retain(|t| refresh || !have.contains_key(&t.1));
    if limit > 0 {
        todo.truncate(limit);
    }
    println!(
        "{} existing rows; {} artists to look up (~{:.1} hours at {:.1}s each)",
        rows.len(),
        todo.len(),
        todo.len() as f64 * gap.as_secs_f64() / 3600.0,
        gap.as_secs_f64()
    );

    let save = |rows: &[Vec<String>]| -> Res<()> {
        let mut sorted: Vec<&Vec<String>> = rows.iter().collect();
        sorted.sort_by(|a, b| parse_plays(&b[14]).cmp(&parse_plays(&a[14])));
        let mut w = writer(&out)?;
        w.write_record(HEADER).map_err(err("write"))?;
        for r in sorted {
            w.write_record(r).map_err(err("write"))?;
        }
        w.flush().map_err(err("write"))
    };

    let mut last = Instant::now() - Duration::from_secs(60);
    let n = todo.len();
    for (i, (name, mbid, tracks, plays)) in todo.into_iter().enumerate() {
        if let Some(f) =
            fetch(&agent, &cache, &mbid, gap, &mut last, offline).and_then(|d| extract(&d))
        {
            let mut row = vec![name.clone(), mbid.clone()];
            row.extend(f);
            row.push(tracks);
            row.push(plays.to_string());
            match have.get(&mbid) {
                Some(&k) => rows[k] = row,
                None => {
                    have.insert(mbid, rows.len());
                    rows.push(row);
                }
            }
        }
        if (i + 1) % 25 == 0 || i + 1 == n {
            println!(
                "[{:>7}/{n}] {}",
                i + 1,
                name.chars().take(40).collect::<String>()
            );
        }
        if (i + 1) % 500 == 0 {
            save(&rows)?; // an interrupted run loses almost nothing
        }
    }
    save(&rows)?;
    println!("Wrote {} ({} artists)", out.display(), rows.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_genre_country_era() {
        let d: Value = serde_json::from_str(
            r#"{"genres":[{"name":"glam rock","count":3},{"name":"art rock","count":9}],"tags":[{"name":"british","count":1}],
                "life-span":{"begin":"1971-01","end":null},"area":{"name":"United Kingdom"},"country":"GB","type":"Group","disambiguation":""}"#,
        )
        .unwrap();
        let e = extract(&d).unwrap();
        assert_eq!(e[0], "art rock"); // most-voted first
        assert_eq!(e[1], "art rock; glam rock");
        assert_eq!(e[3], "GB");
        assert_eq!(e[7], "1971");
        assert_eq!(e[9], "1970s");
    }
}
