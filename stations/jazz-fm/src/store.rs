//! Plays on disk: `plays/YYYY-MM.tsv`, one row per play, `played_at_utc<TAB>track_id`,
//! sorted, so the files only ever grow and diffs are clean.

use crate::extract::fmt_utc;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

const HEADER: &str = "played_at_utc\ttrack_id\tshow\n";

type Key = (String, String);

/// `(played_at_utc, track_id) -> show`. Files written before the show column existed have two
/// columns; the show reads as empty and is filled in when the same play is merged again.
fn read_month(path: &Path) -> io::Result<BTreeMap<Key, String>> {
    let mut map = BTreeMap::new();
    match fs::read_to_string(path) {
        Ok(s) => {
            for line in s.lines().skip(1) {
                let mut f = line.split('\t');
                if let (Some(a), Some(b)) = (f.next(), f.next()) {
                    map.insert((a.to_string(), b.to_string()), f.next().unwrap_or("").to_string());
                }
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    Ok(map)
}

/// Add plays (`unix time`, `track id`, `show`) to the monthly files. Returns how many were new
/// (a play already stored, only gaining its show, is not counted).
pub fn merge(dir: &Path, plays: &[(i64, String, String)]) -> io::Result<usize> {
    let plays_dir = dir.join("plays");
    fs::create_dir_all(&plays_dir)?;
    let mut by_month: BTreeMap<String, Vec<(Key, String)>> = BTreeMap::new();
    for (t, id, show) in plays {
        let s = fmt_utc(*t);
        by_month
            .entry(s[..7].to_string())
            .or_default()
            .push(((s, id.clone()), show.replace(['\t', '\n', '\r'], " ")));
    }
    let mut added = 0;
    for (month, rows) in by_month {
        let path = plays_dir.join(format!("{month}.tsv"));
        let mut map = read_month(&path)?;
        for (key, show) in rows {
            match map.get_mut(&key) {
                Some(existing) => {
                    if existing.is_empty() {
                        *existing = show;
                    }
                }
                None => {
                    map.insert(key, show);
                    added += 1;
                }
            }
        }
        let mut out = String::from(HEADER);
        for ((a, b), show) in &map {
            out.push_str(&format!("{a}\t{b}\t{show}\n"));
        }
        fs::write(&path, out)?;
    }
    Ok(added)
}

/// Every stored play as `(played_at_utc, track_id, show)`, oldest first.
pub fn load_all(dir: &Path) -> io::Result<Vec<(String, String, String)>> {
    let plays_dir = dir.join("plays");
    let mut files: Vec<_> = match fs::read_dir(&plays_dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "tsv"))
            .collect(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e),
    };
    files.sort();
    let mut all = Vec::new();
    for f in files {
        for ((a, b), show) in read_month(&f)? {
            all.push((a, b, show));
        }
    }
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(t: i64, id: &str, show: &str) -> (i64, String, String) {
        (t, id.to_string(), show.to_string())
    }

    #[test]
    fn merge_is_idempotent_and_sorted() {
        let dir = std::env::temp_dir().join(format!("jazzfm-store-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        // 2026-11-13T10:05:00Z, five minutes earlier, and a play on 11 December
        let a = p(1_794_564_000, "t1", "A");
        let b = p(1_794_564_300, "t2", "A");
        let c = p(1_797_000_000, "t3", "B");
        assert_eq!(merge(&dir, &[b.clone(), a.clone()]).unwrap(), 2);
        assert_eq!(merge(&dir, &[a.clone(), b.clone(), c.clone()]).unwrap(), 1);
        assert_eq!(merge(&dir, &[a, b, c]).unwrap(), 0);
        let all = load_all(&dir).unwrap();
        assert_eq!(all.len(), 3);
        assert!(all.windows(2).all(|w| w[0] <= w[1]));
        assert!(dir.join("plays/2026-11.tsv").exists());
        assert!(dir.join("plays/2026-12.tsv").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn old_two_column_files_gain_their_show_without_counting_as_new() {
        let dir = std::env::temp_dir().join(format!("jazzfm-store2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("plays")).unwrap();
        fs::write(
            dir.join("plays/2026-11.tsv"),
            "played_at_utc\ttrack_id\n2026-11-13T10:05:00Z\tt2\n",
        )
        .unwrap();
        assert_eq!(load_all(&dir).unwrap()[0].2, "");
        assert_eq!(merge(&dir, &[p(1_794_564_300, "t2", "Daytimes")]).unwrap(), 0);
        assert_eq!(load_all(&dir).unwrap()[0].2, "Daytimes");
        // an existing show is never overwritten
        assert_eq!(merge(&dir, &[p(1_794_564_300, "t2", "Other")]).unwrap(), 0);
        assert_eq!(load_all(&dir).unwrap()[0].2, "Daytimes");
        let _ = fs::remove_dir_all(&dir);
    }
}
