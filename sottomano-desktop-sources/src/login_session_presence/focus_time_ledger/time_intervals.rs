use chrono::{DateTime, TimeDelta, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval {
    pub from: DateTime<Utc>,
    pub until: DateTime<Utc>,
}

impl Interval {
    #[must_use]
    pub fn length(self) -> TimeDelta {
        (self.until - self.from).max(TimeDelta::zero())
    }

    #[must_use]
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
        |mut scan, interval| {
            if interval.from > scan.cursor {
                scan.gaps.push(Interval {
                    from: scan.cursor,
                    until: interval.from,
                });
            }
            scan.cursor = scan.cursor.max(interval.until);
            scan
        },
    );
    let trailing = (scanned.cursor < until).then_some(Interval {
        from: scanned.cursor,
        until,
    });
    scanned.gaps.into_iter().chain(trailing).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrailingPeriod {
    pub length: TimeDelta,
}

pub const LAST_DAY: TrailingPeriod = TrailingPeriod {
    length: TimeDelta::days(1),
};

pub const LAST_HOUR: TrailingPeriod = TrailingPeriod {
    length: TimeDelta::hours(1),
};

impl TrailingPeriod {
    #[must_use]
    pub fn ending_at(self, now: DateTime<Utc>) -> Interval {
        Interval {
            from: now
                .checked_sub_signed(self.length)
                .unwrap_or(DateTime::<Utc>::MIN_UTC),
            until: now,
        }
    }
}
