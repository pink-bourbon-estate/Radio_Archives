//! Reading plays out of the playlist page, and the small date and robots.txt helpers it needs.
//!
//! The page is a Next.js page with its data in `<script id="__NEXT_DATA__">`. The plays are in
//! `props.initialState.station.data.stationPlayHistory`, a list of
//! `{nowPlayingArtist, nowPlayingTrack, nowPlayingTime, ...}`. The same JSON also carries the
//! play histories of *related* stations, so only that one path is read, and only when the
//! station is Jazz FM. The page embeds just the latest handful of plays (5 when checked).
//! Times are local UK time without a zone, converted to UTC here.

use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Play {
    /// Seconds since the Unix epoch, UTC.
    pub at: i64,
    pub artist: String,
    pub title: String,
}

#[derive(Debug)]
pub struct Extracted {
    pub plays: Vec<Play>,
}

/// The JSON text inside `<script id="__NEXT_DATA__" ...>`.
pub fn next_data(html: &str) -> Option<&str> {
    let p = html.find("id=\"__NEXT_DATA__\"")?;
    let start = html[p..].find('>')? + p + 1;
    let end = html[start..].find("</script>")? + start;
    Some(&html[start..end])
}

fn text(v: Option<&Value>) -> Option<String> {
    v?.as_str().map(|s| s.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|s| !s.is_empty())
}

pub fn extract(json: &str) -> Result<Extracted, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| format!("page data is not valid JSON: {e}"))?;
    let data = v
        .pointer("/props/initialState/station/data")
        .ok_or("no props.initialState.station.data in the page (has the site changed?)")?;
    if data.get("stationCode").and_then(Value::as_str) != Some("jaz") {
        return Err(format!(
            "this page is for station {:?}, not Jazz FM (code \"jaz\")",
            data.get("stationCode")
        ));
    }
    let history = data
        .get("stationPlayHistory")
        .and_then(Value::as_array)
        .ok_or("no stationPlayHistory list in the station data")?;
    let mut plays = Vec::new();
    for item in history {
        let (Some(artist), Some(title), Some(time)) = (
            text(item.get("nowPlayingArtist")),
            text(item.get("nowPlayingTrack")),
            text(item.get("nowPlayingTime")),
        ) else {
            continue;
        };
        let Some((t, naive)) = parse_time(&time) else { continue };
        let at = if naive { london_to_utc(t) } else { t };
        plays.push(Play { at, artist, title });
    }
    plays.sort();
    plays.dedup();
    Ok(Extracted { plays })
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
    // A sample of anything called "playlist" / "tracks" / "songs", which is where plays should be.
    fn sample(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(o) => {
                for (k, x) in o {
                    let kl = k.to_lowercase();
                    if out.len() < 3 && (kl.contains("playlist") || kl.contains("track") || kl.contains("song")) {
                        let mut t = x.to_string();
                        t.truncate(t.char_indices().nth(1500).map_or(t.len(), |(i, _)| i));
                        out.push(format!("{k} = {t}"));
                    } else {
                        sample(x, out);
                    }
                }
            }
            Value::Array(a) => a.iter().take(3).for_each(|x| sample(x, out)),
            _ => {}
        }
    }
    match serde_json::from_str::<Value>(json) {
        Ok(v) => {
            let mut seen = Vec::new();
            keys(&v, &mut seen);
            let mut samples = Vec::new();
            sample(&v, &mut samples);
            format!(
                "{} bytes of JSON; keys seen: {}\nSAMPLES:\n{}",
                json.len(),
                seen.iter().take(30).cloned().collect::<Vec<_>>().join(", "),
                samples.join("\n")
            )
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


/// Day of the month of the last Sunday in a 31-day month (March or October).
fn last_sunday(y: i64, m: i64) -> i64 {
    let days = days_from_civil(y, m, 31);
    let weekday = (days + 4).rem_euclid(7); // 0 = Sunday (1970-01-01 was a Thursday)
    31 - weekday
}

/// Convert a UK wall-clock time (given as if it were UTC seconds) to real UTC. UK summer time
/// runs from 01:00 UTC on the last Sunday of March to 01:00 UTC on the last Sunday of October.
pub fn london_to_utc(local: i64) -> i64 {
    let (y, _, _) = civil_from_days(local.div_euclid(86_400));
    let start = days_from_civil(y, 3, last_sunday(y, 3)) * 86_400 + 3600;
    let end = days_from_civil(y, 10, last_sunday(y, 10)) * 86_400 + 3600;
    let as_summer = local - 3600;
    if as_summer >= start && as_summer < end { as_summer } else { local }
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

    fn page(history: &str) -> String {
        format!(
            r#"{{"props":{{"initialState":{{"station":{{"data":{{"stationId":375,"stationCode":"jaz",
            "stationPlayHistory":{history},
            "stationRelated":[{{"stationId":108,"stationCode":"abs","stationPlayHistory":[
              {{"nowPlayingArtist":"Donna Summer","nowPlayingTrack":"Bad Girls","nowPlayingTime":"2026-11-13 11:53:07"}}]}}]}}}}}}}}}}"#
        )
    }

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
        assert!(parse_time("2026-11-13 11:56:00").unwrap().1, "no zone is reported");
        assert!(parse_time("not a date").is_none());
        assert!(parse_time("2026-13-01T10:05:00Z").is_none());
    }

    #[test]
    fn uk_local_time_becomes_utc() {
        let l = |s: &str| fmt_utc(london_to_utc(parse_time(s).unwrap().0));
        assert_eq!(l("2026-11-13 11:56:00"), "2026-11-13T11:56:00Z"); // winter: same
        assert_eq!(l("2026-07-01 12:00:00"), "2026-07-01T11:00:00Z"); // summer: one hour earlier
        assert_eq!(l("2026-03-29 00:30:00"), "2026-03-29T00:30:00Z"); // before the clocks go forward
        assert_eq!(l("2026-03-29 02:30:00"), "2026-03-29T01:30:00Z"); // after
        assert_eq!(l("2026-10-25 00:30:00"), "2026-10-24T23:30:00Z"); // still summer time
        assert_eq!(l("2026-10-25 02:30:00"), "2026-10-25T02:30:00Z"); // clocks have gone back
        assert_eq!(last_sunday(2026, 3), 29);
        assert_eq!(last_sunday(2026, 10), 25);
    }

    #[test]
    fn next_data_is_cut_out() {
        let html = r#"<html><script id="__NEXT_DATA__" type="application/json">{"a":1}</script></html>"#;
        assert_eq!(next_data(html), Some(r#"{"a":1}"#));
        assert_eq!(next_data("<html></html>"), None);
    }

    #[test]
    fn reads_only_jazz_fm_plays_in_the_real_shape() {
        let j = page(
            r#"[{"nowPlayingTrackId":534106,"nowPlayingTrack":"Laughter and Tears","nowPlayingArtist":"The Womack Sisters","nowPlayingTime":"2026-11-13 11:56:00","nowPlayingDuration":123},
                {"nowPlayingTrackId":1,"nowPlayingTrack":"I Got It Bad ","nowPlayingArtist":"Oscar  Peterson and Ben Webster","nowPlayingTime":"2026-11-13 11:46:11"},
                {"nowPlayingTrack":"No Time","nowPlayingArtist":"Skipped"}]"#,
        );
        let e = extract(&j).unwrap();
        assert_eq!(e.plays.len(), 2, "the related station and the incomplete item are left out");
        assert_eq!(e.plays[0].title, "I Got It Bad");
        assert_eq!(e.plays[0].artist, "Oscar Peterson and Ben Webster");
        assert_eq!(fmt_utc(e.plays[1].at), "2026-11-13T11:56:00Z");
    }

    #[test]
    fn wrong_station_or_shape_is_an_error() {
        let other = page("[]").replace("\"jaz\"", "\"abs\"");
        assert!(extract(&other).unwrap_err().contains("not Jazz FM"));
        assert!(extract(r#"{"props":{"menu":[]}}"#).is_err());
        assert!(extract("{broken").is_err());
        assert!(extract(&page("[]")).unwrap().plays.is_empty());
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
