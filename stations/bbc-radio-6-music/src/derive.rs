//! Build the song database (unique tracks, per-show counts, artists) from the raw plays.

use crate::util::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

struct Track {
    artist: String,
    title: String,
    album: String,
    label: String,
    mbid: String,
    plays: u64,
    first: String,
    last: String,
}

struct Artist {
    name: String,
    mbid: String,
    plays: u64,
    tracks: u64,
}

/// Every plays file under the data folder, oldest first.
///
/// With `include_new`, the small daily top-up files in `data/plays/new/` are read too.
/// The daily refresh writes there, so the committed snapshot in `data/*.csv` (which CI
/// rebuilds and compares on every push) stays exactly in step with `data/plays/`.
pub fn plays_files(data: &Path, include_new: bool) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = Vec::new();
    let mut dirs = vec![data.join("plays")];
    if include_new {
        dirs.push(data.join("plays").join("new"));
    }
    for dir in dirs {
        if let Ok(rd) = std::fs::read_dir(dir) {
            v.extend(rd.flatten().map(|e| e.path()).filter(|p| {
                let n = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                n.starts_with("plays_") && (n.ends_with(".csv") || n.ends_with(".csv.gz"))
            }));
        }
    }
    let single = data.join("plays_raw.csv");
    if single.exists() {
        v.push(single);
    }
    v.sort();
    v
}

pub fn run(args: &Args) -> Res<()> {
    let data = PathBuf::from(args.str_or("data", "data"));
    let out = PathBuf::from(args.str_or("out", &data.to_string_lossy()));
    let files = match args.get("plays") {
        Some(p) => vec![PathBuf::from(p)],
        None => plays_files(&data, args.flag("include-new")),
    };
    if files.is_empty() {
        return Err(format!("no plays files found in {}/plays - run `bbc6 scrape` first, or point --data at the folder", data.display()));
    }

    // (artist, title) -> index into `tracks`, in first-seen order
    let mut tidx: HashMap<(String, String), usize> = HashMap::new();
    let mut tracks: Vec<((String, String), Track)> = Vec::new();
    let mut by_show: HashMap<(String, String, String), usize> = HashMap::new();
    let mut shows: Vec<((String, String, String), u64)> = Vec::new();
    let mut aidx: HashMap<String, usize> = HashMap::new();
    let mut artists: Vec<(String, Artist)> = Vec::new();
    let mut n_plays = 0u64;

    for path in &files {
        println!("Reading {} ...", path.display());
        let mut r = open_csv(path)?;
        let c = Cols::of(&mut r, path)?;
        if !c.has("Artist") || !c.has("Title") {
            return Err(format!(
                "{} does not look like a plays file (no Artist/Title columns)",
                path.display()
            ));
        }
        for rec in r.records() {
            let rec = rec.map_err(err(&format!("{}: bad row", path.display())))?;
            n_plays += 1;
            let (artist, title) = (
                scrub_emails(c.get(&rec, "Artist")),
                scrub_emails(c.get(&rec, "Title")),
            );
            let (album, label) = (
                scrub_emails(c.get(&rec, "Album")),
                scrub_emails(c.get(&rec, "Label")),
            );
            let (artist, title, album, label) = (
                artist.as_ref(),
                title.as_ref(),
                album.as_ref(),
                label.as_ref(),
            );
            let mbid = c.get(&rec, "MusicBrainzArtistID");
            let (date, show) = (c.get(&rec, "Date"), c.get(&rec, "Show"));
            let (na, nt) = (norm(artist), norm(title));
            if na.is_empty() && nt.is_empty() {
                continue;
            }

            match tidx.get(&(na.clone(), nt.clone())) {
                None => {
                    tidx.insert((na.clone(), nt.clone()), tracks.len());
                    tracks.push((
                        (na.clone(), nt.clone()),
                        Track {
                            artist: artist.into(),
                            title: title.into(),
                            album: album.into(),
                            label: label.into(),
                            mbid: mbid.into(),
                            plays: 1,
                            first: date.into(),
                            last: date.into(),
                        },
                    ));
                }
                Some(&i) => {
                    let t = &mut tracks[i].1;
                    t.plays += 1;
                    if date < t.first.as_str() {
                        t.first = date.into();
                    }
                    if date > t.last.as_str() {
                        t.last = date.into();
                    }
                    if t.album.is_empty() {
                        t.album = album.into();
                    }
                    if t.label.is_empty() {
                        t.label = label.into();
                    }
                    if t.mbid.is_empty() {
                        t.mbid = mbid.into();
                    }
                }
            }

            let key = (show.to_string(), na.clone(), nt.clone());
            match by_show.get(&key) {
                Some(&i) => shows[i].1 += 1,
                None => {
                    by_show.insert(key.clone(), shows.len());
                    shows.push((key, 1));
                }
            }

            if !na.is_empty() {
                match aidx.get(&na) {
                    Some(&i) => {
                        let a = &mut artists[i].1;
                        a.plays += 1;
                        if a.mbid.is_empty() {
                            a.mbid = mbid.into();
                        }
                    }
                    None => {
                        aidx.insert(na.clone(), artists.len());
                        artists.push((
                            na,
                            Artist {
                                name: artist.into(),
                                mbid: mbid.into(),
                                plays: 1,
                                tracks: 0,
                            },
                        ));
                    }
                }
            }
        }
    }

    // distinct tracks per artist
    for ((na, _), _) in &tracks {
        if let Some(&i) = aidx.get(na) {
            artists[i].1.tracks += 1;
        }
    }

    std::fs::create_dir_all(&out).map_err(err("cannot create output folder"))?;

    // `tracks` stays in first-seen order so `tidx` remains valid; write via a sorted index list.
    let mut order: Vec<usize> = (0..tracks.len()).collect();
    order.sort_by(|&a, &b| {
        tracks[b]
            .1
            .plays
            .cmp(&tracks[a].1.plays)
            .then_with(|| tracks[a].0.cmp(&tracks[b].0))
    });
    let mut w = writer(&out.join("tracks_unique.csv"))?;
    w.write_record([
        "Artist",
        "Title",
        "Album",
        "Label",
        "Plays",
        "First played",
        "Last played",
        "MusicBrainzArtistID",
    ])
    .map_err(err("write"))?;
    for &i in &order {
        let t = &tracks[i].1;
        w.write_record([
            &t.artist,
            &t.title,
            &t.album,
            &t.label,
            &t.plays.to_string(),
            &t.first,
            &t.last,
            &t.mbid,
        ])
        .map_err(err("write"))?;
    }
    w.flush().map_err(err("write"))?;

    shows.sort_by(|a, b| a.0 .0.cmp(&b.0 .0).then(b.1.cmp(&a.1)));
    let mut w = writer(&out.join("tracks_by_programme.csv"))?;
    w.write_record(["Show", "Artist", "Title", "Plays"])
        .map_err(err("write"))?;
    for ((show, na, nt), n) in &shows {
        if let Some(&i) = tidx.get(&(na.clone(), nt.clone())) {
            let t = &tracks[i].1;
            w.write_record([show.as_str(), &t.artist, &t.title, &n.to_string()])
                .map_err(err("write"))?;
        }
    }
    w.flush().map_err(err("write"))?;

    artists.sort_by(|a, b| b.1.plays.cmp(&a.1.plays).then_with(|| a.0.cmp(&b.0)));
    let mut w = writer(&out.join("artists_unique.csv"))?;
    w.write_record([
        "Artist",
        "MusicBrainzArtistID",
        "Distinct tracks",
        "Total plays",
    ])
    .map_err(err("write"))?;
    for (_, a) in &artists {
        w.write_record([
            &a.name,
            &a.mbid,
            &a.tracks.to_string(),
            &a.plays.to_string(),
        ])
        .map_err(err("write"))?;
    }
    w.flush().map_err(err("write"))?;

    println!("\n{n_plays:>10} plays read");
    println!(
        "{:>10} distinct tracks   -> tracks_unique.csv",
        tracks.len()
    );
    println!(
        "{:>10} show/track rows   -> tracks_by_programme.csv",
        shows.len()
    );
    println!(
        "{:>10} distinct artists  -> artists_unique.csv",
        artists.len()
    );
    println!("Written to {}", out.display());
    Ok(())
}
