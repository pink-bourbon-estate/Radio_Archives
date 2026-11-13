mod derive;
mod enrich;
mod pages;
mod playlists;
mod scrape;
mod site;
mod util;

use std::path::PathBuf;
use util::*;

const HELP: &str = "\
bbc6 - an offline database of every track played on BBC Radio 6 Music

USAGE: bbc6 <command> [options]

COMMANDS
  probe       Check the BBC feeds work: fetch one day and one tracklist
  scrape      Download schedules + tracklists, add new plays to data/plays/
  derive      Build tracks_unique / tracks_by_programme / artists_unique from the plays
  enrich      Add genre/country/era to artists from MusicBrainz (slow, resumable)
  playlists   Write per-show and per-genre playlist files   [all|genre|programme]
  site        Build the website (data files and pre-built pages) from the plays and artist data
  find        Search the song database:  bbc6 find roxy music

COMMON OPTIONS
  --data DIR        Data folder (default: data)
  --offline         Never use the network; read the cache only

scrape:    --start YYYY-MM-DD (2007-01-01)  --end YYYY-MM-DD (yesterday)
           --service PID (p00fzl65 = 6 Music)  --workers 1-6 (3)  --gap SECS (0.25)
           --retry-empty   re-check episodes that previously had no tracks
probe:     --date YYYY-MM-DD (2016-03-17)
enrich:    --contact EMAIL_OR_URL (required)  --min-plays N  --limit N  --refresh
derive:    --include-new   also read the daily top-ups in data/plays/new/
site:      --src DIR (site) --out DIR (public) --origin URL --base /path/
playlists: --min-plays N  --format csv|txt  --minimal  --report  --out DIR
find:      --limit N (25)
";

fn find(args: &Args) -> Res<()> {
    let data = PathBuf::from(args.str_or("data", "data"));
    let words: Vec<String> = args.positional[1..].iter().map(|w| norm(w)).collect();
    if words.is_empty() {
        return Err("usage: bbc6 find <words to search for>".into());
    }
    let limit: usize = args.num("limit", 25usize)?;
    let path = data.join("tracks_unique.csv");
    let mut r = open_csv(&path).map_err(|e| format!("{e} (run `bbc6 derive` first)"))?;
    let c = Cols::of(&mut r, &path)?;
    let (mut shown, mut total) = (0, 0);
    for rec in r.records().flatten() {
        let hay = norm(&format!(
            "{} {}",
            c.get(&rec, "Artist"),
            c.get(&rec, "Title")
        ));
        if words.iter().all(|w| hay.contains(w.as_str())) {
            total += 1;
            if shown < limit {
                shown += 1;
                println!(
                    "{:>5} plays  {} - {}  [{} to {}]",
                    c.get(&rec, "Plays"),
                    c.get(&rec, "Artist"),
                    c.get(&rec, "Title"),
                    c.get(&rec, "First played"),
                    c.get(&rec, "Last played")
                );
            }
        }
    }
    println!(
        "{total} matches{}",
        if total > shown {
            format!(" (showing the {shown} most played; use --limit)")
        } else {
            String::new()
        }
    );
    Ok(())
}

fn main() {
    let tokens: Vec<String> = std::env::args().skip(1).collect();
    let result = Args::parse(&tokens).and_then(|args| {
        if args.flag("help") || args.positional.is_empty() {
            print!("{HELP}");
            return Ok(());
        }
        match args.positional[0].as_str() {
            "probe" => scrape::probe(&args),
            "scrape" => scrape::run(&args),
            "derive" => derive::run(&args),
            "enrich" => enrich::run(&args),
            "playlists" => playlists::run(&args),
            "site" => site::run(&args),
            "find" => find(&args),
            other => Err(format!("unknown command '{other}' (try `bbc6 --help`)")),
        }
    });
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
