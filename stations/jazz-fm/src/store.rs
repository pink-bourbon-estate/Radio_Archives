//! Plays on disk: `plays/YYYY-MM.tsv`, one row per play, `played_at_utc<TAB>track_id`,
//! sorted, so the files only ever grow and diffs are clean.

use crate::extract::fmt_utc;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::Path;

const HEADER: &str = "played_at_utc\ttrack_id\n";

fn read_month(path: &Path) -> io::Result<BTreeSet<(String, String)>> {
    let mut set = BTreeSet::new();
    match fs::read_to_string(path) {
        Ok(s) => {
            for line in s.lines().skip(1) {
                if let Some((a, b)) = line.split_once('\t') {
                    set.insert((a.to_string(), b.to_string()));
                }
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    Ok(set)
}

/// Add plays (`unix time`, `track id`) to the monthly files. Returns how many were new.
pub fn merge(dir: &Path, plays: &[(i64, String)]) -> io::Result<usize> {
    let plays_dir = dir.join("plays");
    fs::create_dir_all(&plays_dir)?;
    let mut by_month: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for (t, id) in plays {
        let s = fmt_utc(*t);
        by_month.entry(s[..7].to_string()).or_default().push((s, id.clone()));
    }
    let mut added = 0;
    for (month, rows) in by_month {
        let path = plays_dir.join(format!("{month}.tsv"));
        let mut set = read_month(&path)?;
        for r in rows {
            if set.insert(r) {
                added += 1;
            }
        }
        let mut out = String::from(HEADER);
        for (a, b) in &set {
            out.push_str(&format!("{a}\t{b}\n"));
        }
        fs::write(&path, out)?;
    }
    Ok(added)
}

/// Every stored play as `(played_at_utc, track_id)`, oldest first.
pub fn load_all(dir: &Path) -> io::Result<Vec<(String, String)>> {
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
        all.extend(read_month(&f)?);
    }
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_is_idempotent_and_sorted() {
        let dir = std::env::temp_dir().join(format!("jazzfm-store-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        // 2026-11-13T10:00:00Z and a play in the next month
        let a = (1_794_564_000, "t1".to_string());
        let b = (1_794_564_300, "t2".to_string());
        let c = (1_797_000_000, "t3".to_string()); // 11 December 2026
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
}
