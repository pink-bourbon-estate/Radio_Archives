//! Reading plays out of the JSON the Planet Radio listen API returns, plus the small date helpers
//! that needs.
//!
//! The API (`https://listenapi.planetradio.co.uk/api9.2/events/jaz/{cursor}/{count}`) answers
//! with a JSON list of the `count` plays immediately before `cursor`, newest first. Each item has
//! `nowPlayingArtist`, `nowPlayingTrack` and `nowPlayingTime`. Times are UK local time without a
//! zone, converted to UTC here. The reply is either a bare list or an object wrapping the list
//! under `data`, `events` or `items`; all of those are accepted.

use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Play {
    /// Seconds since the Unix epoch, UTC.
    pub at: i64,
    pub artist: String,
    pub title: String,
}

fn text(v: Option<&Value>) -> Option<String> {
    v?.as_str().map(|s| s.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|s| !s.is_empty())
}

/// The plays in one API reply. Items missing an artist, title or readable time are skipped.
/// Sorted oldest first, without exact duplicates. Errors only when the reply is not JSON or has
/// no list in it at all (which would mean the API has changed).
pub fn plays_from_json(json: &str) -> Result<Vec<Play>, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| format!("reply is not valid JSON: {e}"))?;
    let list = match &v {
        Value::Array(a) => a,
        Value::Object(o) => ["data", "events", "items"]
            .iter()
            .find_map(|k| o.get(*k).and_then(Value::as_array))
            .ok_or("reply is an object with no data/events/items list (has the API changed?)")?,
        _ => return Err("reply is neither a list nor an object".into()),
    };
    let mut plays = Vec::new();
    for item in list {
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
    Ok(plays)
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

/// UK summer time runs from 01:00 UTC on the last Sunday of March to 01:00 UTC on the last
/// Sunday of October. Returns that range, in Unix seconds, for a year.
fn summer_time(y: i64) -> (i64, i64) {
    let start = days_from_civil(y, 3, last_sunday(y, 3)) * 86_400 + 3600;
    let end = days_from_civil(y, 10, last_sunday(y, 10)) * 86_400 + 3600;
    (start, end)
}

/// Convert a UK wall-clock time (given as if it were UTC seconds) to real UTC.
pub fn london_to_utc(local: i64) -> i64 {
    let (y, _, _) = civil_from_days(local.div_euclid(86_400));
    let (start, end) = summer_time(y);
    let as_summer = local - 3600;
    if as_summer >= start && as_summer < end { as_summer } else { local }
}

/// The reverse: real UTC to UK wall-clock time, as seconds.
pub fn utc_to_london(t: i64) -> i64 {
    let (y, _, _) = civil_from_days(t.div_euclid(86_400));
    let (start, end) = summer_time(y);
    if t >= start && t < end { t + 3600 } else { t }
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
    fn reads_a_bare_list_in_the_real_shape() {
        let j = r#"[{"nowPlayingTrackId":534106,"nowPlayingTrack":"Laughter and Tears","nowPlayingArtist":"The Womack Sisters","nowPlayingTime":"2026-11-13 11:56:00"},
                    {"nowPlayingTrack":"I Got It Bad ","nowPlayingArtist":"Oscar  Peterson and Ben Webster","nowPlayingTime":"2026-11-13 11:46:11"},
                    {"nowPlayingTrack":"No Time","nowPlayingArtist":"Skipped"}]"#;
        let p = plays_from_json(j).unwrap();
        assert_eq!(p.len(), 2, "the incomplete item is left out");
        assert_eq!(p[0].title, "I Got It Bad");
        assert_eq!(p[0].artist, "Oscar Peterson and Ben Webster");
        assert_eq!(fmt_utc(p[1].at), "2026-11-13T11:56:00Z");
    }

    #[test]
    fn reads_a_wrapped_list_and_rejects_other_shapes() {
        let w = r#"{"data":[{"nowPlayingTrack":"A","nowPlayingArtist":"B","nowPlayingTime":"2026-11-13 11:56:00"}]}"#;
        assert_eq!(plays_from_json(w).unwrap().len(), 1);
        assert!(plays_from_json("[]").unwrap().is_empty());
        assert!(plays_from_json(r#"{"menu":[]}"#).is_err());
        assert!(plays_from_json("{broken").is_err());
        assert!(plays_from_json("42").is_err());
    }
}
