pub mod content_log_lines;
pub mod controller_log_lines;
pub mod game_process_log_lines;
pub mod hardware_update_log_lines;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use winnow::{
    Parser, Result,
    ascii::{dec_int, dec_uint},
    combinator::{delimited, separated_pair, terminated},
    token::take,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SteamAppId {
    pub number: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLineShape<'line> {
    Stamped {
        logged_at: NaiveDateTime,
        message: &'line str,
    },
    Continuation(&'line str),
}

impl<'line> From<&'line str> for LogLineShape<'line> {
    fn from(line: &'line str) -> Self {
        let mut remaining = line;
        match bracketed_timestamp.parse_next(&mut remaining) {
            Ok(logged_at) => LogLineShape::Stamped {
                logged_at,
                message: remaining,
            },
            Err(_) => LogLineShape::Continuation(line),
        }
    }
}

fn fixed_width_number(width: usize) -> impl FnMut(&mut &str) -> Result<u32> {
    move |input: &mut &str| take(width).and_then(dec_uint).parse_next(input)
}

fn calendar_date(input: &mut &str) -> Result<NaiveDate> {
    (
        take(4_usize).and_then(dec_int),
        '-',
        fixed_width_number(2),
        '-',
        fixed_width_number(2),
    )
        .verify_map(|(year, _, month, _, day)| NaiveDate::from_ymd_opt(year, month, day))
        .parse_next(input)
}

fn wall_clock_time(input: &mut &str) -> Result<NaiveTime> {
    (
        fixed_width_number(2),
        ':',
        fixed_width_number(2),
        ':',
        fixed_width_number(2),
    )
        .verify_map(|(hour, _, minute, _, second)| NaiveTime::from_hms_opt(hour, minute, second))
        .parse_next(input)
}

fn bracketed_timestamp(input: &mut &str) -> Result<NaiveDateTime> {
    terminated(
        delimited(
            '[',
            separated_pair(calendar_date, ' ', wall_clock_time),
            ']',
        ),
        ' ',
    )
    .map(|(date, time)| NaiveDateTime::new(date, time))
    .parse_next(input)
}

pub fn app_id(input: &mut &str) -> Result<SteamAppId> {
    dec_uint
        .map(|number| SteamAppId { number })
        .parse_next(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_timestamp_prefix_splits_off_the_message() {
        let expected_moment = NaiveDate::from_ymd_opt(2026, 11, 20)
            .and_then(|date| date.and_hms_opt(11, 14, 57))
            .expect("valid moment");
        assert_eq!(
            LogLineShape::from("[2026-11-20 11:14:57] Current download rate: 0.000 Mbps"),
            LogLineShape::Stamped {
                logged_at: expected_moment,
                message: "Current download rate: 0.000 Mbps",
            }
        );
    }

    #[test]
    fn continuation_lines_carry_no_timestamp() {
        assert_eq!(
            LogLineShape::from("  path: /dev/hidraw10"),
            LogLineShape::Continuation("  path: /dev/hidraw10")
        );
        assert!(matches!(
            LogLineShape::from("[2026-13-40 99:00:00] impossible"),
            LogLineShape::Continuation(_)
        ));
    }
}
