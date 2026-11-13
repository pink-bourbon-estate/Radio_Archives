//! Which show was on, worked out from the day and time (the playlist data has no show).
//!
//! This is a hard-coded weekly timetable, so it will drift when the station changes its
//! schedule. It was checked against 1,776 plays collected by hand (see `jazzfm import`).
//! Input is UK wall-clock time (seconds, as if UTC).

pub fn show_for(local: i64) -> &'static str {
    let days = local.div_euclid(86_400);
    let wd = (days + 3).rem_euclid(7); // 0 = Monday (1970-01-01 was a Thursday)
    let m = local.rem_euclid(86_400) / 60; // minute of the day
    let h = |x: i64| x * 60;
    match wd {
        0..=4 => {
            if m < h(1) {
                "Sentimental Jazz"
            } else if m < h(2) {
                "The Jazz Guitar"
            } else if m < 390 {
                "The World's Greatest Jazz, Soul and Blues"
            } else if m < h(10) {
                "Jazz FM Breakfast with Nigel Williams"
            } else if m < h(14) {
                "Daytimes with Danielle Perry"
            } else if m < h(18) {
                "Afternoons with Simon Phillips"
            } else if m < h(19) {
                "Jazz FM's Greatest with Mark Walker"
            } else if m < h(21) {
                "Dinner Jazz with Mark Walker"
            } else if m < h(22) {
                match wd {
                    0 => "Blues & Boogie with David Freeman",
                    1 => "The Playlist",
                    2 => "Move On Up",
                    3 => "Pathways with Lou Paley",
                    _ => "Transatlantic Jazz",
                }
            } else {
                "Late Night China Moses"
            }
        }
        5 => match m {
            m if m < h(6) => "The World's Greatest Jazz, Soul and Blues",
            m if m < h(9) => "Breakfast at the Weekend with Nia Visser",
            m if m < h(10) => "Jazz Shapers with Elliot Moss",
            m if m < h(14) => "Nigel Williams Saturday Show",
            m if m < h(16) => "YolanDa Brown on Saturday",
            m if m < h(18) => "The Ronnie Scott's Radio Show with Ian Shaw",
            m if m < h(19) => "The Jon Cleary Show",
            m if m < h(21) => "Dinner Jazz with Mark Walker",
            _ => "Jazztronics with Dom Servini",
        },
        _ => match m {
            m if m < h(6) => "The World's Greatest Jazz, Soul and Blues",
            m if m < h(9) => "Breakfast at the Weekend with Nia Visser",
            m if m < h(10) => "Mica Millar's Jazz Voices",
            m if m < h(13) => "Tony Minvielle",
            m if m < h(15) => "Robbie Vincent",
            m if m < h(17) => "The Leo Green Show",
            m if m < h(19) => "Chris Philips",
            m if m < h(21) => "Dinner Jazz with Mark Walker",
            m if m < h(22) => "Future Jazz with Ruth Fisher",
            _ => "Jazz Chill with Rouhangeze",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::{london_to_utc, parse_time, utc_to_london};

    fn at(s: &str) -> &'static str {
        show_for(parse_time(s).unwrap().0)
    }

    #[test]
    fn weekday_and_weekend_slots() {
        // 2026-11-13 is a Friday
        assert_eq!(at("2026-11-13 00:30:00"), "Sentimental Jazz");
        assert_eq!(at("2026-11-13 06:29:00"), "The World's Greatest Jazz, Soul and Blues");
        assert_eq!(at("2026-11-13 06:30:00"), "Jazz FM Breakfast with Nigel Williams");
        assert_eq!(at("2026-11-13 12:10:25"), "Daytimes with Danielle Perry");
        assert_eq!(at("2026-11-13 21:30:00"), "Transatlantic Jazz");
        assert_eq!(at("2026-11-09 21:30:00"), "Blues & Boogie with David Freeman"); // Monday
        assert_eq!(at("2026-11-07 15:00:00"), "YolanDa Brown on Saturday");
        assert_eq!(at("2026-11-08 23:50:00"), "Jazz Chill with Rouhangeze"); // Sunday
        assert_eq!(at("2026-11-08 12:00:00"), "Tony Minvielle");
    }

    #[test]
    fn utc_to_local_round_trips() {
        for s in ["2026-11-13 11:56:00", "2026-07-01 12:00:00", "2026-03-29 03:30:00", "2026-10-25 00:30:00"] {
            let local = parse_time(s).unwrap().0;
            let utc = london_to_utc(local);
            assert_eq!(utc_to_london(utc), local, "{s}");
        }
    }
}
