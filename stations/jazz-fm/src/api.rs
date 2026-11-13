//! Fetching plays straight from the Planet Radio listen API, the same JSON endpoint the
//! "View more" button on the public Jazz FM playlist page calls.
//!
//!   GET https://listenapi.planetradio.co.uk/api9.2/events/jaz/{cursor}/{count}
//!
//! `cursor` is a UK local time (`YYYY-MM-DD HH:MM:SS`, URL-encoded); the reply holds the `count`
//! plays immediately before it. To go back in time, the oldest time in a reply minus one second
//! becomes the next cursor. See `docs/JAZZ-FM.md` for why this is used and how politely.

use crate::extract::{self, Play};
use std::time::Duration;

const BASE: &str = "https://listenapi.planetradio.co.uk/api9.2/events/jaz";
const USER_AGENT: &str = "RadioArchivesBot/0.2 (+https://github.com/pink-bourbon-estate/Radio_Archives; \
                          unofficial non-commercial archive; one small batch of requests per week)";
/// Plays per request. The site itself asks for 24; larger sizes are accepted by the API.
pub const BATCH: usize = 48;
/// Pause between requests.
const DELAY: Duration = Duration::from_millis(250);
/// Hard stop on requests in one run, so a fault can never turn into a loop.
const MAX_REQUESTS: usize = 400;

pub struct Collected {
    pub plays: Vec<Play>,
    pub requests: usize,
    /// Why the walk stopped, for the log.
    pub stopped: String,
}

/// `2026-11-13 05:20:30`, URL-encoded, for a UK wall-clock time given as seconds.
fn cursor(local: i64) -> String {
    extract::fmt_utc(local).replace('T', "%20").replace(':', "%3A").trim_end_matches('Z').to_string()
}

/// Walk backwards from `now_utc` until `stop_at_utc` (plays at or before it are not needed) or
/// until the API has no more history. Whatever was read before an error is still returned.
pub fn collect(now_utc: i64, stop_at_utc: i64) -> Result<Collected, String> {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(20)).user_agent(USER_AGENT).build();
    let mut out = Collected { plays: Vec::new(), requests: 0, stopped: String::new() };
    let mut at = now_utc; // the cursor, as UTC
    while out.requests < MAX_REQUESTS {
        let url = format!("{BASE}/{}/{BATCH}", cursor(extract::utc_to_london(at)));
        out.requests += 1;
        let body = match agent.get(&url).set("Accept", "application/json").call() {
            Ok(r) => r.into_string().map_err(|e| format!("could not read the reply to {url}: {e}"))?,
            Err(e) if out.plays.is_empty() => return Err(format!("could not fetch {url}: {e}")),
            Err(e) => {
                out.stopped = format!("stopped early: {e}");
                break;
            }
        };
        let batch = extract::plays_from_json(&body)?;
        let Some(oldest) = batch.first().map(|p| p.at) else {
            out.stopped = "the API has no earlier plays".into();
            break;
        };
        out.plays.extend(batch);
        if oldest <= stop_at_utc {
            out.stopped = "reached plays that are already stored".into();
            break;
        }
        if oldest >= at {
            out.stopped = "the API did not move back in time".into();
            break;
        }
        at = oldest - 1;
        std::thread::sleep(DELAY);
    }
    if out.stopped.is_empty() {
        out.stopped = format!("hit the safety limit of {MAX_REQUESTS} requests");
    }
    out.plays.sort();
    out.plays.dedup();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_is_the_urlencoded_local_time() {
        let t = extract::parse_time("2026-11-13 05:20:30").unwrap().0;
        assert_eq!(cursor(t), "2026-11-13%2005%3A20%3A30");
    }
}
