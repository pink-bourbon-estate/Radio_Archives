//! The shared catalogue: every artist and track once, referenced by ID from every station.
//!
//! IDs come from the *normalised* name, so the same song played on two stations (or spelled
//! "The Beatles" / "Beatles", "Sigur Rós" / "Sigur Ros", "Song (Remastered 2011)" / "Song")
//! gets the same ID without any lookup or network call. Outside identifiers (MusicBrainz IDs,
//! ISRC codes) are *attributes* filled in later, never part of the ID, so an ID never changes.
//!
//! On disk it is two readable, sorted, tab-separated files (`artists.tsv`, `tracks.tsv`), so
//! changes show up cleanly in git.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

/// Lower-case, and fold common accented letters to plain ASCII.
pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars().flat_map(|c| c.to_lowercase()) {
        match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' => out.push('a'),
            'æ' => out.push_str("ae"),
            'ç' | 'č' | 'ć' => out.push('c'),
            'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ę' | 'ě' => out.push('e'),
            'ì' | 'í' | 'î' | 'ï' | 'ī' => out.push('i'),
            'ñ' | 'ń' | 'ň' => out.push('n'),
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' => out.push('o'),
            'œ' => out.push_str("oe"),
            'ś' | 'š' | 'ş' => out.push('s'),
            'ß' => out.push_str("ss"),
            'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' => out.push('u'),
            'ý' | 'ÿ' => out.push('y'),
            'ž' | 'ź' | 'ż' => out.push('z'),
            '’' | '‘' | '`' => out.push('\''),
            _ => out.push(c),
        }
    }
    out
}

/// Letters and digits only, single spaces, "&" and "+" read as "and", apostrophes dropped.
fn squash(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in fold(s).chars() {
        if c.is_alphanumeric() {
            out.push(c);
        } else if c == '\'' {
            // "don't" and "dont" are the same word
        } else if c == '&' || c == '+' {
            out.push_str(" and ");
        } else {
            out.push(' ');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Normalised artist: no leading "The", and only the main artist before "feat." / "ft." / "featuring".
pub fn normalise_artist(s: &str) -> String {
    let squashed = squash(s);
    let words: Vec<&str> = squashed.split(' ').filter(|w| !w.is_empty()).collect();
    let cut = words
        .iter()
        .position(|w| matches!(*w, "feat" | "ft" | "featuring"))
        .filter(|&i| i > 0)
        .unwrap_or(words.len());
    let mut words = &words[..cut];
    if words.len() > 1 && words[0] == "the" {
        words = &words[1..];
    }
    words.join(" ")
}

fn is_noise(inner: &str) -> bool {
    let low = fold(inner);
    const SUBSTR: &[&str] = &[
        "remaster",
        "feat",
        "radio edit",
        "single version",
        "album version",
        "bonus track",
    ];
    if SUBSTR.iter().any(|n| low.contains(n)) {
        return true;
    }
    squash(inner)
        .split(' ')
        .any(|w| matches!(w, "mono" | "stereo" | "ft"))
}

/// Drop "(Remastered 2011)", "[feat. X]", " - Mono Version" style suffixes, which describe the
/// release rather than the song.
fn strip_noise(t: &str) -> String {
    let chars: Vec<char> = t.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '(' || c == '[' {
            let close = if c == '(' { ')' } else { ']' };
            if let Some(j) = chars[i + 1..].iter().position(|&x| x == close) {
                let inner: String = chars[i + 1..i + 1 + j].iter().collect();
                if is_noise(&inner) {
                    i += j + 2;
                    continue;
                }
            }
        }
        out.push(c);
        i += 1;
    }
    if let Some(p) = out.rfind(" - ") {
        if is_noise(&out[p + 3..]) {
            out.truncate(p);
        }
    }
    out
}

pub fn normalise_title(s: &str) -> String {
    squash(&strip_noise(s))
}

fn fnv64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Artist ID from the normalised name: `a` + 12 hex digits. The format is frozen: changing it
/// would change every ID.
pub fn artist_id(normalised_name: &str) -> String {
    format!("a{:012x}", fnv64(normalised_name) & 0xffff_ffff_ffff)
}

/// Track ID from the artist ID and the normalised title: `t` + 16 hex digits.
pub fn track_id(artist_id: &str, normalised_title: &str) -> String {
    format!("t{:016x}", fnv64(&format!("{artist_id}\u{0}{normalised_title}")))
}

fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The artist as it should be shown: the raw name without a trailing "feat. ..." part.
fn display_artist(raw: &str) -> String {
    let low = raw.to_ascii_lowercase(); // same byte offsets as `raw`
    let cut = [" feat. ", " feat ", " ft. ", " ft ", " featuring "]
        .iter()
        .filter_map(|m| low.find(m))
        .filter(|&p| p > 0)
        .min()
        .unwrap_or(raw.len());
    clean(&raw[..cut])
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Artist {
    pub id: String,
    pub name: String,
    /// MusicBrainz artist ID, once known.
    pub mbid: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Track {
    pub id: String,
    pub artist_id: String,
    pub title: String,
    /// ISRC, once known (looked up lazily, because MusicBrainz is rate-limited).
    pub isrc: String,
    /// MusicBrainz recording ID, once known.
    pub mbid: String,
}

#[derive(Default, Debug)]
pub struct Catalogue {
    pub artists: BTreeMap<String, Artist>,
    pub tracks: BTreeMap<String, Track>,
}

impl Catalogue {
    /// Load `artists.tsv` and `tracks.tsv` from `dir`. Missing files mean an empty catalogue.
    pub fn load(dir: &Path) -> io::Result<Self> {
        let mut cat = Catalogue::default();
        for line in read_rows(&dir.join("artists.tsv"))? {
            let f: Vec<&str> = line.split('\t').collect();
            let get = |i: usize| f.get(i).copied().unwrap_or("").to_string();
            if get(0).is_empty() {
                continue;
            }
            cat.artists.insert(
                get(0),
                Artist { id: get(0), name: get(1), mbid: get(2) },
            );
        }
        for line in read_rows(&dir.join("tracks.tsv"))? {
            let f: Vec<&str> = line.split('\t').collect();
            let get = |i: usize| f.get(i).copied().unwrap_or("").to_string();
            if get(0).is_empty() {
                continue;
            }
            cat.tracks.insert(
                get(0),
                Track { id: get(0), artist_id: get(1), title: get(2), isrc: get(3), mbid: get(4) },
            );
        }
        Ok(cat)
    }

    /// Write both files, sorted by ID so git diffs only show real changes.
    pub fn save(&self, dir: &Path) -> io::Result<()> {
        fs::create_dir_all(dir)?;
        let mut a = String::from("id\tname\tmbid\n");
        for x in self.artists.values() {
            a.push_str(&format!("{}\t{}\t{}\n", x.id, clean(&x.name), clean(&x.mbid)));
        }
        fs::write(dir.join("artists.tsv"), a)?;
        let mut t = String::from("id\tartist_id\ttitle\tisrc\tmbid\n");
        for x in self.tracks.values() {
            t.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\n",
                x.id,
                x.artist_id,
                clean(&x.title),
                clean(&x.isrc),
                clean(&x.mbid)
            ));
        }
        fs::write(dir.join("tracks.tsv"), t)
    }

    /// Find or add the artist and track, returning `(artist_id, track_id)`.
    /// `None` when either name is empty once normalised.
    pub fn intern(&mut self, artist: &str, title: &str) -> Option<(String, String)> {
        let na = normalise_artist(artist);
        let nt = normalise_title(title);
        if na.is_empty() || nt.is_empty() {
            return None;
        }
        let aid = artist_id(&na);
        let tid = track_id(&aid, &nt);
        self.artists.entry(aid.clone()).or_insert_with(|| Artist {
            id: aid.clone(),
            name: display_artist(artist),
            mbid: String::new(),
        });
        self.tracks.entry(tid.clone()).or_insert_with(|| Track {
            id: tid.clone(),
            artist_id: aid.clone(),
            title: clean(title),
            ..Track::default()
        });
        Some((aid, tid))
    }

    pub fn artist_name(&self, id: &str) -> &str {
        self.artists.get(id).map(|a| a.name.as_str()).unwrap_or("?")
    }
}

fn read_rows(path: &Path) -> io::Result<Vec<String>> {
    match fs::read_to_string(path) {
        Ok(s) => Ok(s.lines().skip(1).map(str::to_string).collect()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(a: &str, t: &str) -> (String, String) {
        let mut c = Catalogue::default();
        c.intern(a, t).unwrap()
    }

    #[test]
    fn spellings_of_one_song_share_ids() {
        assert_eq!(ids("The Beatles", "Hey Jude"), ids("Beatles", "hey jude"));
        assert_eq!(ids("Sigur Rós", "Hoppípolla"), ids("Sigur Ros", "Hoppipolla"));
        assert_eq!(ids("Simon & Garfunkel", "Mrs. Robinson"), ids("Simon and Garfunkel", "Mrs Robinson"));
        assert_eq!(ids("Miles Davis", "So What (Remastered 2011)"), ids("Miles Davis", "So What"));
        assert_eq!(ids("Miles Davis", "So What - Mono Version"), ids("Miles Davis", "So What"));
        assert_eq!(ids("Prince", "Don’t Stop"), ids("Prince", "Dont Stop"));
        assert_eq!(ids("Daft Punk feat. Pharrell", "Get Lucky"), ids("Daft Punk", "Get Lucky"));
    }

    #[test]
    fn different_things_get_different_ids() {
        assert_ne!(ids("Miles Davis", "So What").1, ids("John Coltrane", "So What").1);
        assert_ne!(ids("Miles Davis", "So What").1, ids("Miles Davis", "Blue in Green").1);
        // a live version is a different recording, so it is kept apart
        assert_ne!(ids("Miles Davis", "So What").1, ids("Miles Davis", "So What (Live)").1);
        // "Monologue" is not the noise word "mono"
        assert_ne!(ids("A", "Song (Monologue)").1, ids("A", "Song").1);
    }

    #[test]
    fn id_format_is_frozen() {
        // If this fails, every stored ID in every station would change. Do not "fix" the test.
        assert_eq!(artist_id("miles davis"), artist_id("miles davis"));
        assert_eq!(fnv64(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv64("a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(artist_id("a").len(), 13);
        assert_eq!(track_id("a1", "x").len(), 17);
    }

    #[test]
    fn empty_names_are_rejected() {
        let mut c = Catalogue::default();
        assert!(c.intern("", "x").is_none());
        assert!(c.intern("x", "  ").is_none());
        assert!(c.intern("!!!", "x").is_none());
    }

    #[test]
    fn first_spelling_is_kept_for_display() {
        let mut c = Catalogue::default();
        let (aid, tid) = c.intern("Daft Punk feat. Pharrell Williams", "Get Lucky (feat. Pharrell)").unwrap();
        c.intern("DAFT PUNK", "get lucky").unwrap();
        assert_eq!(c.artists.len(), 1);
        assert_eq!(c.tracks.len(), 1);
        assert_eq!(c.artist_name(&aid), "Daft Punk");
        assert_eq!(c.tracks[&tid].title, "Get Lucky (feat. Pharrell)");
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = std::env::temp_dir().join(format!("catalogue-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut c = Catalogue::default();
        c.intern("Miles Davis", "So What").unwrap();
        c.intern("Nina Simone", "Feeling Good").unwrap();
        c.tracks.values_mut().next().unwrap().isrc = "USSM15900001".into();
        c.save(&dir).unwrap();
        let back = Catalogue::load(&dir).unwrap();
        assert_eq!(back.artists, c.artists);
        assert_eq!(back.tracks, c.tracks);
        // saving again gives byte-identical files (stable, sorted)
        let before = fs::read_to_string(dir.join("tracks.tsv")).unwrap();
        back.save(&dir).unwrap();
        assert_eq!(before, fs::read_to_string(dir.join("tracks.tsv")).unwrap());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn loading_a_missing_directory_is_empty() {
        let c = Catalogue::load(Path::new("/nonexistent/catalogue-dir")).unwrap();
        assert!(c.artists.is_empty() && c.tracks.is_empty());
    }
}
