//! Small shared helpers: argument parsing, dates, text folding, CSV/gzip readers.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub type Res<T> = Result<T, String>;

pub fn err<E: std::fmt::Display>(ctx: &str) -> impl FnOnce(E) -> String + '_ {
    move |e| format!("{ctx}: {e}")
}

// ---------------------------------------------------------------- arguments

/// Flags that take no value. Everything else written `--name value` takes one.
const BOOL_FLAGS: &[&str] = &[
    "offline",
    "minimal",
    "report",
    "refresh",
    "retry-empty",
    "include-new",
    "help",
];

#[derive(Default)]
pub struct Args {
    pub positional: Vec<String>,
    values: HashMap<String, String>,
    flags: Vec<String>,
}

impl Args {
    pub fn parse(tokens: &[String]) -> Res<Args> {
        let mut a = Args::default();
        let mut i = 0;
        while i < tokens.len() {
            let t = &tokens[i];
            if let Some(name) = t.strip_prefix("--") {
                if let Some((k, v)) = name.split_once('=') {
                    a.values.insert(k.to_string(), v.to_string());
                } else if BOOL_FLAGS.contains(&name) {
                    a.flags.push(name.to_string());
                } else {
                    i += 1;
                    let v = tokens.get(i).ok_or(format!("--{name} needs a value"))?;
                    a.values.insert(name.to_string(), v.clone());
                }
            } else {
                a.positional.push(t.clone());
            }
            i += 1;
        }
        Ok(a)
    }
    pub fn get(&self, k: &str) -> Option<&str> {
        self.values.get(k).map(|s| s.as_str())
    }
    pub fn str_or(&self, k: &str, d: &str) -> String {
        self.get(k).unwrap_or(d).to_string()
    }
    pub fn flag(&self, k: &str) -> bool {
        self.flags.iter().any(|f| f == k)
    }
    pub fn num<T: std::str::FromStr>(&self, k: &str, d: T) -> Res<T> {
        match self.get(k) {
            None => Ok(d),
            Some(v) => v
                .parse()
                .map_err(|_| format!("--{k}: '{v}' is not a valid number")),
        }
    }
}

// -------------------------------------------------------------------- dates

/// A calendar day, stored as days since 1970-01-01 (proleptic Gregorian).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Day(pub i64);

impl Day {
    pub fn from_ymd(y: i64, m: i64, d: i64) -> Day {
        let y = if m <= 2 { y - 1 } else { y };
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = y - era * 400;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        Day(era * 146097 + doe - 719468)
    }
    pub fn ymd(self) -> (i64, i64, i64) {
        let z = self.0 + 719468;
        let era = if z >= 0 { z } else { z - 146096 } / 146097;
        let doe = z - era * 146097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        (yoe + era * 400 + if m <= 2 { 1 } else { 0 }, m, d)
    }
    pub fn parse(s: &str) -> Res<Day> {
        let bad = || format!("'{s}' is not a date in YYYY-MM-DD form");
        let p: Vec<&str> = s.trim().split('-').collect();
        if p.len() != 3 {
            return Err(bad());
        }
        let n: Vec<i64> = p
            .iter()
            .map(|x| x.parse().map_err(|_| bad()))
            .collect::<Res<_>>()?;
        let d = Day::from_ymd(n[0], n[1], n[2]);
        if d.ymd() != (n[0], n[1], n[2]) {
            return Err(bad());
        }
        Ok(d)
    }
    pub fn today() -> Day {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Day((secs / 86400) as i64)
    }
    pub fn iso(self) -> String {
        let (y, m, d) = self.ymd();
        format!("{y:04}-{m:02}-{d:02}")
    }
}

// --------------------------------------------------------------------- text

/// Fold text for comparison: trim, lowercase, straighten curly quotes, collapse spaces.
pub fn norm(s: &str) -> String {
    let s = s
        .trim()
        .to_lowercase()
        .replace(['\u{2019}', '\u{2018}'], "'")
        .replace(['\u{201c}', '\u{201d}'], "\"");
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Make a string safe to use as a file name on Windows, macOS and Linux.
pub fn sanitize_filename(name: &str, max: usize, empty: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| {
            !matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') && !c.is_control()
        })
        .collect();
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() {
        empty.to_string()
    } else {
        cleaned.chars().take(max).collect()
    }
}

/// Replace anything that looks like an email address with "[email removed]".
/// BBC tracklists occasionally contain a person's address in a title or artist
/// field; the database should not republish those.
pub fn scrub_emails(s: &str) -> Cow<'_, str> {
    if !s.contains('@') {
        return Cow::Borrowed(s);
    }
    let ch: Vec<char> = s.chars().collect();
    let local_ok = |c: char| c.is_alphanumeric() || "._%+-".contains(c);
    let domain_ok = |c: char| c.is_alphanumeric() || ".-".contains(c);
    let mut out = String::with_capacity(s.len());
    let mut i = 0; // start of text not yet copied
    let mut at = 0;
    while at < ch.len() {
        if ch[at] == '@' {
            let mut a = at;
            while a > i && local_ok(ch[a - 1]) {
                a -= 1;
            }
            let mut b = at + 1;
            while b < ch.len() && domain_ok(ch[b]) {
                b += 1;
            }
            while b > at + 1 && matches!(ch[b - 1], '.' | '-') {
                b -= 1; // trailing punctuation belongs to the sentence, not the address
            }
            let domain: String = ch[at + 1..b].iter().collect();
            let tld_ok = domain
                .rsplit_once('.')
                .map(|(_, t)| t.chars().count() >= 2 && t.chars().all(|c| c.is_alphabetic()))
                .unwrap_or(false);
            if a < at && tld_ok {
                out.extend(&ch[i..a]);
                out.push_str("[email removed]");
                i = b;
                at = b;
                continue;
            }
        }
        at += 1;
    }
    out.extend(&ch[i..]);
    Cow::Owned(out)
}

// ---------------------------------------------------------------------- csv

/// Open a .csv or .csv.gz file as a CSV reader.
pub fn open_csv(path: &Path) -> Res<csv::Reader<Box<dyn Read>>> {
    let f = File::open(path).map_err(err(&format!("cannot open {}", path.display())))?;
    let inner: Box<dyn Read> = if path.extension().and_then(|e| e.to_str()) == Some("gz") {
        Box::new(flate2::read::MultiGzDecoder::new(BufReader::new(f)))
    } else {
        Box::new(BufReader::new(f))
    };
    Ok(csv::ReaderBuilder::new().flexible(true).from_reader(inner))
}

/// Column positions by header name, so files can be read without caring about column order.
pub struct Cols(HashMap<String, usize>);

impl Cols {
    pub fn of(r: &mut csv::Reader<Box<dyn Read>>, path: &Path) -> Res<Cols> {
        let h = r
            .headers()
            .map_err(err(&format!("{}: bad header", path.display())))?;
        Ok(Cols(
            h.iter()
                .enumerate()
                .map(|(i, n)| (n.trim_start_matches('\u{feff}').to_string(), i))
                .collect(),
        ))
    }
    /// Value of the named column in a record ("" if the column or value is missing).
    pub fn get<'a>(&self, rec: &'a csv::StringRecord, name: &str) -> &'a str {
        self.0.get(name).and_then(|&i| rec.get(i)).unwrap_or("")
    }
    pub fn has(&self, name: &str) -> bool {
        self.0.contains_key(name)
    }
}

pub fn writer(path: &Path) -> Res<csv::Writer<File>> {
    csv::Writer::from_path(path).map_err(err(&format!("cannot write {}", path.display())))
}

pub fn parse_plays(s: &str) -> u64 {
    s.trim().parse().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_round_trip() {
        for s in [
            "1970-01-01",
            "2007-01-01",
            "2016-03-17",
            "2024-02-29",
            "2026-11-09",
        ] {
            assert_eq!(Day::parse(s).unwrap().iso(), s);
        }
        assert!(Day::parse("2023-02-29").is_err());
        assert!(Day::parse("nope").is_err());
        assert_eq!(
            Day::parse("2016-03-18").unwrap().0 - Day::parse("2016-03-17").unwrap().0,
            1
        );
    }

    #[test]
    fn norm_folds_case_space_and_quotes() {
        assert_eq!(norm("  The  FALL "), "the fall");
        assert_eq!(norm("Don\u{2019}t Stop"), "don't stop");
    }

    #[test]
    fn emails_are_scrubbed() {
        assert_eq!(scrub_emails("plain text"), "plain text");
        assert_eq!(scrub_emails("someone@Example.Com"), "[email removed]");
        assert_eq!(
            scrub_emails("Brearley Music, ASCAP, a.b@example.com."),
            "Brearley Music, ASCAP, [email removed]."
        );
        assert_eq!(
            scrub_emails("From x@host to bob@example.org (session)"),
            "From x@host to [email removed] (session)"
        );
        assert_eq!(scrub_emails("@home and a@b"), "@home and a@b");
        assert_eq!(
            scrub_emails("two a@example.org b@example.net"),
            "two [email removed] [email removed]"
        );
    }

    #[test]
    fn filenames_are_safe() {
        assert_eq!(
            sanitize_filename("Radio 1/6 Music: Live?", 150, "x"),
            "Radio 16 Music Live"
        );
        assert_eq!(sanitize_filename("???", 150, "Unnamed"), "Unnamed");
    }

    #[test]
    fn args_parse() {
        let t: Vec<String> = ["scrape", "--start", "2016-03-01", "--offline", "--gap=0.5"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let a = Args::parse(&t).unwrap();
        assert_eq!(a.positional, vec!["scrape"]);
        assert_eq!(a.get("start"), Some("2016-03-01"));
        assert!(a.flag("offline"));
        assert_eq!(a.num("gap", 0.25f64).unwrap(), 0.5);
    }
}
