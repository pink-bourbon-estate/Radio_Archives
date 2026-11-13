//! Turn the song database into playlist files: one per show, and one per broad genre.
//! Output is Artist,Title,Album (+ Plays and genre) so it can be imported into
//! Soundiiz, TuneMyMusic, MusicTransfer and similar services.

use crate::util::*;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

/// Broad genre buckets. The FIRST bucket with a keyword found in the MusicBrainz
/// genre wins, so specific buckets come before general ones.
const GENRE_GROUPS: &[(&str, &[&str])] = &[
    (
        "Gospel & Christian",
        &["gospel", "christian", "ccm", "worship", "hymn"],
    ),
    (
        "Hip Hop & Rap",
        &[
            "hip hop",
            "rap",
            "grime",
            "boom bap",
            "trap",
            "drill",
            "conscious hip hop",
        ],
    ),
    (
        "Reggae, Ska & Dub",
        &[
            "reggae",
            "dub",
            "dancehall",
            "ska",
            "rocksteady",
            "roots reggae",
        ],
    ),
    (
        "Soul, Funk & R&B",
        &[
            "soul",
            "funk",
            "r&b",
            "motown",
            "doo-wop",
            "neo soul",
            "quiet storm",
        ],
    ),
    (
        "Jazz",
        &[
            "jazz",
            "bebop",
            "bossa nova",
            "swing",
            "big band",
            "hard bop",
        ],
    ),
    ("Blues", &["blues"]),
    (
        "Classical & Orchestral",
        &[
            "classical",
            "orchestral",
            "opera",
            "baroque",
            "romanticism",
            "chamber music",
            "minimalism",
            "modern classical",
        ],
    ),
    (
        "Folk & Americana",
        &[
            "folk",
            "americana",
            "bluegrass",
            "singer-songwriter",
            "country",
        ],
    ),
    (
        "Punk & Hardcore",
        &["punk", "hardcore", "oi!", "riot grrrl"],
    ),
    ("Metal", &["metal", "doom", "thrash", "sludge", "grindcore"]),
    (
        "New Wave & Post-Punk",
        &["new wave", "post-punk", "no wave", "coldwave", "darkwave"],
    ),
    (
        "Ambient & Experimental",
        &[
            "ambient",
            "drone",
            "noise",
            "experimental",
            "avant-garde",
            "musique concrete",
            "field recording",
            "sound art",
        ],
    ),
    (
        "Electronic & Dance",
        &[
            "electronic",
            "techno",
            "house",
            "trance",
            "dubstep",
            "drum and bass",
            "jungle",
            "breakbeat",
            "idm",
            "electro",
            "downtempo",
            "dance",
            "disco",
            "synth-pop",
            "electropop",
            "big beat",
            "trip hop",
            "garage",
            "grime",
            "electronica",
            "edm",
            "club",
            "acid house",
            "gabber",
        ],
    ),
    (
        "World & Latin",
        &[
            "afrobeat",
            "latin",
            "salsa",
            "mpb",
            "highlife",
            "world",
            "flamenco",
            "reggaeton",
            "bhangra",
            "fado",
            "cumbia",
            "soca",
            "calypso",
        ],
    ),
    (
        "Alternative & Indie",
        &[
            "alternative rock",
            "indie",
            "shoegaze",
            "dream pop",
            "art rock",
            "art pop",
            "britpop",
            "grunge",
            "college rock",
            "jangle pop",
            "chamber pop",
            "baroque pop",
            "lo-fi",
            "c86",
            "krautrock",
        ],
    ),
    ("Pop", &["pop"]),
    (
        "Rock",
        &[
            "rock",
            "rock and roll",
            "rockabilly",
            "glam",
            "psychedelic",
            "garage rock",
            "progressive rock",
            "hard rock",
            "surf",
        ],
    ),
];
const FALLBACK: &str = "Other";

pub fn classify(genre: &str) -> &'static str {
    let g = genre.to_lowercase();
    GENRE_GROUPS
        .iter()
        .find(|(_, kws)| kws.iter().any(|k| g.contains(k)))
        .map(|(n, _)| *n)
        .unwrap_or(FALLBACK)
}

struct Opts {
    min_plays: u64,
    txt: bool,
    minimal: bool,
}

struct Row {
    artist: String,
    title: String,
    album: String,
    plays: u64,
    extra: String, // MicroGenre or Genre
}

fn write_playlist(
    dir: &Path,
    stem: &str,
    rows: &mut [Row],
    o: &Opts,
    extra_col: &str,
    max_len: usize,
) -> Res<PathBuf> {
    rows.sort_by_key(|r| std::cmp::Reverse(r.plays)); // stable: ties keep their input order
    let path = dir.join(format!(
        "{}.{}",
        sanitize_filename(stem, max_len, "Unnamed"),
        if o.txt { "txt" } else { "csv" }
    ));
    if o.txt {
        let body: String = rows
            .iter()
            .map(|r| format!("{} - {}\n", r.artist, r.title))
            .collect();
        fs::write(&path, body).map_err(err("write"))?;
    } else {
        let mut w = writer(&path)?;
        if o.minimal {
            w.write_record(["Artist", "Title", "Album"])
                .map_err(err("write"))?;
            for r in rows.iter() {
                w.write_record([&r.artist, &r.title, &r.album])
                    .map_err(err("write"))?;
            }
        } else if extra_col.is_empty() {
            w.write_record(["Artist", "Title", "Album", "Plays"])
                .map_err(err("write"))?;
            for r in rows.iter() {
                w.write_record([&r.artist, &r.title, &r.album, &r.plays.to_string()])
                    .map_err(err("write"))?;
            }
        } else {
            w.write_record(["Artist", "Title", "Album", "Plays", extra_col])
                .map_err(err("write"))?;
            for r in rows.iter() {
                w.write_record([
                    &r.artist,
                    &r.title,
                    &r.album,
                    &r.plays.to_string(),
                    &r.extra,
                ])
                .map_err(err("write"))?;
            }
        }
        w.flush().map_err(err("write"))?;
    }
    Ok(path)
}

fn need(p: &Path, hint: &str) -> Res<()> {
    if p.exists() {
        Ok(())
    } else {
        Err(format!("{} not found - {hint}", p.display()))
    }
}

pub fn run(args: &Args) -> Res<()> {
    let data = PathBuf::from(args.str_or("data", "data"));
    let out = PathBuf::from(args.str_or("out", &data.join("playlists").to_string_lossy()));
    let which = args.positional.get(1).map(|s| s.as_str()).unwrap_or("all");
    if !matches!(which, "all" | "genre" | "programme") {
        return Err("usage: bbc6 playlists [all|genre|programme]".into());
    }
    let o = Opts {
        min_plays: args.num("min-plays", 1u64)?,
        txt: match args.str_or("format", "csv").as_str() {
            "csv" => false,
            "txt" => true,
            f => return Err(format!("--format must be csv or txt, not '{f}'")),
        },
        minimal: args.flag("minimal"),
    };

    let tracks_path = data.join("tracks_unique.csv");
    let artists_path = data.join("artists_enriched.csv");
    need(&tracks_path, "run `bbc6 derive` first")?;

    // MBID -> genre, and artist name -> genre
    let mut by_mbid: HashMap<String, String> = HashMap::new();
    let mut by_name: HashMap<String, String> = HashMap::new();
    if artists_path.exists() {
        let mut r = open_csv(&artists_path)?;
        let c = Cols::of(&mut r, &artists_path)?;
        for rec in r.records().flatten() {
            let g = c.get(&rec, "PrimaryGenre").trim();
            if g.is_empty() {
                continue;
            }
            by_mbid.insert(
                c.get(&rec, "MusicBrainzArtistID").trim().to_string(),
                g.to_string(),
            );
            by_name.insert(c.get(&rec, "Artist").to_string(), g.to_string());
        }
        println!("{} artists have a genre", by_mbid.len());
    } else {
        println!("(no artists_enriched.csv - genre playlists will be skipped; run `bbc6 enrich`)");
    }

    // album lookup for per-show playlists + the unique tracks themselves
    let mut r = open_csv(&tracks_path)?;
    let c = Cols::of(&mut r, &tracks_path)?;
    let mut album: HashMap<(String, String), String> = HashMap::new();
    let mut genre_buckets: BTreeMap<&str, Vec<Row>> = BTreeMap::new();
    let mut unmatched: Vec<Row> = Vec::new();
    let mut other_report: HashMap<String, usize> = HashMap::new();
    for rec in r.records().flatten() {
        let (artist, title, alb) = (
            c.get(&rec, "Artist"),
            c.get(&rec, "Title"),
            c.get(&rec, "Album"),
        );
        album.insert((artist.to_string(), title.to_string()), alb.to_string());
        let plays = parse_plays(c.get(&rec, "Plays"));
        if plays < o.min_plays {
            continue;
        }
        let row = |extra: &str| Row {
            artist: artist.into(),
            title: title.into(),
            album: alb.into(),
            plays,
            extra: extra.into(),
        };
        match by_mbid.get(c.get(&rec, "MusicBrainzArtistID").trim()) {
            None => unmatched.push(row("")),
            Some(g) => {
                let bucket = classify(g);
                if bucket == FALLBACK {
                    *other_report.entry(g.clone()).or_default() += 1;
                }
                genre_buckets.entry(bucket).or_default().push(row(g));
            }
        }
    }

    if matches!(which, "all" | "genre") && !by_mbid.is_empty() {
        let dir = out.join("by_genre");
        fs::create_dir_all(&dir).map_err(err("cannot create folder"))?;
        let mut n = 0;
        for (bucket, rows) in genre_buckets.iter_mut() {
            write_playlist(&dir, bucket, rows, &o, "MicroGenre", 150)?;
            n += 1;
        }
        if !unmatched.is_empty() {
            write_playlist(&dir, "Unmatched", &mut unmatched, &o, "", 150)?;
            n += 1;
        }
        println!("Wrote {n} genre playlists to {}", dir.display());
        if args.flag("report") && !other_report.is_empty() {
            let mut v: Vec<_> = other_report.into_iter().collect();
            v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            println!("\nGenres landing in 'Other' (most common first):");
            for (g, n) in v.iter().take(40) {
                println!("  {n:6}  {g}");
            }
        }
    }

    if matches!(which, "all" | "programme") {
        let p = data.join("tracks_by_programme.csv");
        need(&p, "run `bbc6 derive` first")?;
        let mut r = open_csv(&p)?;
        let c = Cols::of(&mut r, &p)?;
        let mut by_show: BTreeMap<String, Vec<Row>> = BTreeMap::new();
        for rec in r.records().flatten() {
            let plays = parse_plays(c.get(&rec, "Plays"));
            if plays < o.min_plays {
                continue;
            }
            let show = c.get(&rec, "Show").trim();
            let show = if show.is_empty() {
                "Unknown Show"
            } else {
                show
            };
            let (artist, title) = (c.get(&rec, "Artist"), c.get(&rec, "Title"));
            by_show.entry(show.to_string()).or_default().push(Row {
                artist: artist.into(),
                title: title.into(),
                album: album
                    .get(&(artist.to_string(), title.to_string()))
                    .cloned()
                    .unwrap_or_default(),
                plays,
                extra: by_name.get(artist).cloned().unwrap_or_default(),
            });
        }
        let dir = out.join("by_programme");
        fs::create_dir_all(&dir).map_err(err("cannot create folder"))?;
        let mut used: HashMap<String, usize> = HashMap::new();
        let n = by_show.len();
        for (show, rows) in by_show.iter_mut() {
            let base = sanitize_filename(show, 150, "Unnamed Show");
            let k = used.entry(base.clone()).or_default();
            *k += 1;
            let stem = if *k == 1 {
                base
            } else {
                format!("{base} ({k})")
            };
            write_playlist(&dir, &stem, rows, &o, "Genre", 160)?;
        }
        println!("Wrote {n} programme playlists to {}", dir.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genres_map_to_buckets() {
        assert_eq!(classify("christian hardcore"), "Gospel & Christian");
        assert_eq!(classify("folk rock"), "Folk & Americana");
        assert_eq!(classify("blues rock"), "Blues");
        assert_eq!(classify("art rock"), "Alternative & Indie");
        assert_eq!(classify("krautrock"), "Alternative & Indie");
        assert_eq!(classify("kayōkyoku"), "Other");
    }
}
