use chrono::{DateTime, Days, Local, NaiveDateTime, NaiveTime, TimeDelta, TimeZone, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval {
    pub from: DateTime<Utc>,
    pub until: DateTime<Utc>,
}

impl Interval {
    pub fn length(self) -> TimeDelta {
        (self.until - self.from).max(TimeDelta::zero())
    }

    pub fn intersect(self, other: Interval) -> Option<Interval> {
        let overlap = Interval {
            from: self.from.max(other.from),
            until: self.until.min(other.until),
        };
        (overlap.from < overlap.until).then_some(overlap)
    }
}

pub(super) fn overlaps_with(
    sorted: &[Interval],
    interval: Interval,
) -> impl Iterator<Item = Interval> + '_ {
    sorted[sorted.partition_point(|candidate| candidate.until <= interval.from)..]
        .iter()
        .take_while(move |candidate| candidate.from < interval.until)
        .filter_map(move |candidate| candidate.intersect(interval))
}

pub(super) fn merged(mut intervals: Vec<Interval>) -> Vec<Interval> {
    intervals.sort_by_key(|interval| interval.from);
    intervals.into_iter().fold(Vec::new(), |mut merged, next| {
        match merged.last_mut() {
            Some(last) if next.from <= last.until => last.until = last.until.max(next.until),
            _ => merged.push(next),
        }
        merged
    })
}

struct GapScan {
    cursor: DateTime<Utc>,
    gaps: Vec<Interval>,
}

pub(super) fn gaps_between(
    covered: &[Interval],
    from: DateTime<Utc>,
    until: DateTime<Utc>,
) -> Vec<Interval> {
    let scanned = covered.iter().fold(
        GapScan {
            cursor: from,
            gaps: Vec::new(),
        },
        |scan, interval| {
            let gap = (interval.from > scan.cursor).then_some(Interval {
                from: scan.cursor,
                until: interval.from,
            });
            GapScan {
                cursor: scan.cursor.max(interval.until),
                gaps: scan.gaps.into_iter().chain(gap).collect(),
            }
        },
    );
    let trailing = (scanned.cursor < until).then_some(Interval {
        from: scanned.cursor,
        until,
    });
    scanned.gaps.into_iter().chain(trailing).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WakingWindow {
    pub start: NaiveTime,
    pub end: NaiveTime,
}

impl WakingWindow {
    pub fn around(self, now: DateTime<Utc>) -> Interval {
        let local = now.with_timezone(&Local);
        let today = local.date_naive();
        let yesterday = today.checked_sub_days(Days::new(1)).unwrap_or(today);
        let tomorrow = today.checked_add_days(Days::new(1)).unwrap_or(today);
        let (start_day, end_day) = if self.start <= self.end {
            (today, today)
        } else if local.time() < self.end {
            (yesterday, today)
        } else {
            (today, tomorrow)
        };
        Interval {
            from: local_at(start_day.and_time(self.start), now),
            until: local_at(end_day.and_time(self.end), now),
        }
    }
}

fn local_at(naive: NaiveDateTime, fallback: DateTime<Utc>) -> DateTime<Utc> {
    Local
        .from_local_datetime(&naive)
        .earliest()
        .or_else(|| {
            Local
                .from_local_datetime(&(naive + TimeDelta::hours(1)))
                .earliest()
        })
        .map_or(fallback, |at| at.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overnight_waking_windows_wrap_around_midnight() {
        let waking = WakingWindow {
            start: NaiveTime::from_hms_opt(10, 0, 0).unwrap(),
            end: NaiveTime::from_hms_opt(2, 0, 0).unwrap(),
        };
        let now = Local
            .with_ymd_and_hms(2026, 3, 10, 1, 0, 0)
            .unwrap()
            .with_timezone(&Utc);
        let window = waking.around(now);
        assert_eq!(
            window.from.with_timezone(&Local).naive_local(),
            "2026-03-09T10:00:00".parse().unwrap()
        );
        assert_eq!(
            window.until.with_timezone(&Local).naive_local(),
            "2026-03-10T02:00:00".parse().unwrap()
        );
    }
}
