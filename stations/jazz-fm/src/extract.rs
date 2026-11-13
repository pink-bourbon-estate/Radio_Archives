//! Reading plays out of the playlist page, and the small date and robots.txt helpers it needs.
//!
//! The playlist page is a Next.js page, so its data sits in a `<script id="__NEXT_DATA__">`
//! JSON block. Its exact shape is not documented, so instead of hard-coding one path the
//! extractor walks the whole JSON and accepts any object that has an artist, a title and a
//! time under a handful of common key names. If nothing matches, the caller fails loudly.

use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Play {
    /// Seconds since the Unix epoch, UTC.
    pub at: i64,
    pub artist: String,
    pub title: String,
}

pub struct Extracted {
    pub plays: Vec<Play>,
    /// Some times had no time zone and were read as UTC.
    pub assumed_utc: bool,
}

/// The JSON text inside `<script id="__NEXT_DATA__" ...>`.
pub fn next_data(html: &str) -> Option<&str> {
    let p = html.find("id=\"__NEXT_DATA__\"")?;
    let start = html[p..].find('>')? + p + 1;
    let end = html[start..].find("</script>")? + start;
    Some(&html[start..end])
}

fn key(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase()
}

const ARTIST: &[&str] = &["artist", "artistname", "performer", "by"];
const TITLE: &[&str] = &["title", "trackname", "track", "song", "songtitle", "name"];
const TIME: &[&str] = &[
    "playedat", "startedat", "starttime", "startdatetime", "start", "time", "timestamp",
    "datetime", "date", "playtime", "onair", "starts",
];
const NESTED: &[&str] = &["track", "song", "item", "entry", "metadata", "music"];

fn field<'a>(o: &'a serde_json::Map<String, Value>, aliases: &[&str]) -> Option<&'a Value> {
    aliases
        .iter()
        .find_map(|a| o.iter().find(|(k, _)| key(k) == *a).map(|(_, v)| v))
}

fn text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.trim().to_string()).filter(|s| !s.is_empty()),
        Value::Object(o) => o.get("name").and_then(text),
        _ => None,
    }
}

fn to_epoch(v: &Value, assumed_utc: &mut bool) -> Option<i64> {
    let from_number = |n: f64| -> i64 {
        // milliseconds look like 1.7e12; seconds like 1.7e9
        if n > 1e11 { (n / 1000.0) as i64 } else { n as i64 }
    };
    match v {
        Value::Number(n) => n.as_f64().map(from_number),
        Value::String(s) => {
            let s = s.trim();
            if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) {
                return s.parse::<f64>().ok().map(from_number);
            }
            let (t, naive) = parse_time(s)?;
            if naive {
                *assumed_utc = true;
            }
            Some(t)
        }
        _ => None,
    }
}

fn as_play(o: &serde_json::Map<String, Value>, assumed_utc: &mut bool) -> Option<Play> {
    // The object itself, or a track object nested inside it.
    let mut candidates: Vec<&serde_json::Map<String, Value>> = vec![o];
    for (k, v) in o {
        if let Value::Object(inner) = v {
            if NESTED.contains(&key(k).as_str()) {
                candidates.push(inner);
            }
        }
    }
    let (artist, title) = candidates.iter().find_map(|c| {
        let a = field(c, ARTIST).and_then(text)?;
        let t = field(c, TITLE).and_then(text)?;
        Some((a, t))
    })?;
    let at = candidates
        .iter()
        .find_map(|c| field(c, TIME).and_then(|v| to_epoch(v, assumed_utc)))?;
    Some(Play { at, artist, title })
}

fn walk(v: &Value, out: &mut Vec<Play>, assumed_utc: &mut bool) {
    match v {
        Value::Object(o) => {
            if let Some(p) = as_play(o, assumed_utc) {
                out.push(p);
            }
            for x in o.values() {
                walk(x, out, assumed_utc);
            }
        }
        Value::Array(a) => {
            for x in a {
                walk(x, out, assumed_utc);
            }
        }
        _ => {}
    }
}

pub fn extract(json: &str) -> Result<Extracted, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| format!("page data is not valid JSON: {e}"))?;
    let mut plays = Vec::new();
    let mut assumed_utc = false;
    walk(&v, &mut plays, &mut assumed_utc);
    plays.sort();
    plays.dedup();
    Ok(Extracted { plays, assumed_utc })
}

/// A short description of the JSON, for the error message when no plays were found.
pub fn describe(json: &str) -> String {
    fn keys(v: &Value, seen: &mut Vec<String>) {
        match v {
            Value::Object(o) => {
                for (k, x) in o {
                    if seen.len() < 80 && !seen.contains(k) {
                        seen.push(k.clone());
                    }
                    keys(x, seen);
                }
            }
            Value::Array(a) => a.iter().take(3).for_each(|x| keys(x, seen)),
            _ => {}
        }
    }
    match serde_json::from_str::<Value>(json) {
        Ok(v) => {
            let mut seen = Vec::new();
            keys(&v, &mut seen);
            format!("{} bytes of JSON; keys seen: {}", json.len(), seen.join(", "))
        }
        Err(e) => format!("not JSON: {e}"),
    }
}

// ---- dates -----------------------------------------------------------------------------

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `2026-11-13T10:05:00Z` for a Unix time.
pub fn fmt_utc(t: i64) -> String {
    let (y, m, d) = civil_from_days(t.div_euclid(86_400));
    let s = t.rem_euclid(86_400);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", s / 3600, s % 3600 / 60, s % 60)
}

/// Parse `YYYY-MM-DDTHH:MM[:SS[.fff]]` with `Z`, `+01:00`, `+0100` or `+01`, or none.
/// Returns the Unix time and whether the time zone was missing (read as UTC).
pub fn parse_time(s: &str) -> Option<(i64, bool)> {
    let s = s.trim();
    let b = s.as_bytes();
    if b.len() < 16 {
        return None;
    }
    let num = |a: usize, n: usize| -> Option<i64> {
        let t = s.get(a..a + n)?;
        if !t.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        t.parse::<i64>().ok()
    };
    let y = num(0, 4)?;
    let mo = num(5, 2)?;
    let d = num(8, 2)?;
    let h = num(11, 2)?;
    let mi = num(14, 2)?;
    if b[4] != b'-' || b[7] != b'-' || (b[10] != b'T' && b[10] != b' ') || b[13] != b':' {
        return None;
    }
    let mut i = 16;
    let mut sec = 0;
    if b.get(i).copied() == Some(b':') {
        sec = num(i + 1, 2)?;
        i += 3;
    }
    if b.get(i).copied() == Some(b'.') {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
    }
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || sec > 60 {
        return None;
    }
    let mut off = 0;
    let mut naive = false;
    match b.get(i).copied() {
        None => naive = true,
        Some(b'Z') | Some(b'z') => {}
        Some(c) if c == b'+' || c == b'-' => {
            let sign = if c == b'+' { 1 } else { -1 };
            let oh = num(i + 1, 2)?;
            let rest = s.get(i + 3..)?;
            let om = if rest.starts_with(':') {
                num(i + 4, 2)?
            } else if rest.len() >= 2 {
                num(i + 3, 2)?
            } else {
                0
            };
            off = sign * (oh * 3600 + om * 60);
        }
        _ => return None,
    }
    Some((days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + sec - off, naive))
}

// ---- robots.txt ------------------------------------------------------------------------

/// Whether `path` may be fetched by us, from a robots.txt body. Looks at the `*` group and any
/// group naming `radioarchivesbot`; the longest matching rule wins and Allow wins a tie.
pub fn robots_allows(robots: &str, path: &str) -> bool {
    let mut applies = false;
    let mut in_agents = false;
    let mut best: (usize, bool) = (0, true);
    for line in robots.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let Some((k, v)) = line.split_once(':') else { continue };
        let (k, v) = (k.trim().to_ascii_lowercase(), v.trim());
        match k.as_str() {
            "user-agent" => {
                if !in_agents {
                    applies = false;
                }
                in_agents = true;
                let a = v.to_ascii_lowercase();
                if a == "*" || a.contains("radioarchivesbot") {
                    applies = true;
                }
            }
            "allow" | "disallow" => {
                in_agents = false;
                if applies && !v.is_empty() && path.starts_with(v) {
                    let allow = k == "allow";
                    if v.len() > best.0 || (v.len() == best.0 && allow) {
                        best = (v.len(), allow);
                    }
                }
            }
            _ => {}
        }
    }
    best.1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_parse_and_format() {
        assert_eq!(fmt_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(fmt_utc(951_868_800), "2000-03-01T00:00:00Z");
        let (t, naive) = parse_time("2026-11-13T10:05:00Z").unwrap();
        assert!(!naive);
        assert_eq!(fmt_utc(t), "2026-11-13T10:05:00Z");
        assert_eq!(fmt_utc(parse_time("2026-07-01T10:05:00+01:00").unwrap().0), "2026-07-01T09:05:00Z");
        assert_eq!(fmt_utc(parse_time("2026-07-01T10:05:00.250-0530").unwrap().0), "2026-07-01T15:35:00Z");
        assert_eq!(fmt_utc(parse_time("2024-02-29T23:59").unwrap().0), "2024-02-29T23:59:00Z");
        assert!(parse_time("2026-07-01 10:05:00").unwrap().1, "no zone is reported");
        assert!(parse_time("not a date").is_none());
        assert!(parse_time("2026-13-01T10:05:00Z").is_none());
    }

    #[test]
    fn next_data_is_cut_out() {
        let html = r#"<html><script id="__NEXT_DATA__" type="application/json">{"a":1}</script></html>"#;
        assert_eq!(next_data(html), Some(r#"{"a":1}"#));
        assert_eq!(next_data("<html></html>"), None);
    }

    #[test]
    fn finds_plays_in_a_flat_list() {
        let j = r#"{"props":{"pageProps":{"playlist":[
            {"artist":"Miles Davis","title":"So What","playedAt":"2026-11-13T10:05:00Z"},
            {"artist":"Nina Simone","title":"Feeling Good","playedAt":"2026-11-13T10:00:00Z"},
            {"artist":"Nina Simone","title":"Feeling Good","playedAt":"2026-11-13T10:00:00Z"}
        ]}}}"#;
        let e = extract(j).unwrap();
        assert_eq!(e.plays.len(), 2, "exact duplicates collapse");
        assert_eq!(e.plays[0].artist, "Nina Simone");
        assert!(!e.assumed_utc);
    }

    #[test]
    fn finds_plays_with_nested_tracks_and_other_key_names() {
        let j = r#"{"x":{"items":[
            {"startTime":1794564300,"track":{"name":"So What","artist":{"name":"Miles Davis"}}},
            {"time":"2026-11-13 09:55:00","song":"Blue in Green","artist_name":"Bill Evans"},
            {"startedAt":1794563700000,"trackName":"Naima","performer":"John Coltrane"}
        ]}}"#;
        let e = extract(j).unwrap();
        assert_eq!(e.plays.len(), 3);
        assert!(e.assumed_utc, "the time without a zone is flagged");
        assert!(e.plays.iter().any(|p| p.title == "So What" && p.artist == "Miles Davis"));
        assert!(e.plays.iter().any(|p| p.title == "Naima" && p.at == 1_794_563_700));
    }

    #[test]
    fn unrelated_json_gives_no_plays() {
        let e = extract(r#"{"props":{"menu":[{"title":"Home","link":"/"}]}}"#).unwrap();
        assert!(e.plays.is_empty());
        assert!(extract("{broken").is_err());
        assert!(describe(r#"{"props":{"menu":[]}}"#).contains("props"));
    }

    #[test]
    fn robots_rules() {
        let r = "User-agent: *\nDisallow: /private\nAllow: /private/open\n\nUser-agent: other\nDisallow: /\n";
        assert!(robots_allows(r, "/jazz-fm/playlist"));
        assert!(!robots_allows(r, "/private/x"));
        assert!(robots_allows(r, "/private/open/y"));
        assert!(!robots_allows("User-agent: *\nDisallow: /\n", "/jazz-fm/playlist"));
        assert!(!robots_allows("User-agent: RadioArchivesBot\nDisallow: /jazz-fm\n", "/jazz-fm/playlist"));
        assert!(robots_allows("", "/anything"));
    }
}
