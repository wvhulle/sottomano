use std::{
    collections::HashMap,
    fmt::{self, Display, Formatter},
    mem,
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use crate::clock::Clock;

static LOAD_TALLY: LazyLock<Mutex<LoadTally>> = LazyLock::new(Mutex::default);

static ALLOCATION_COUNT: AtomicU64 = AtomicU64::new(0);

static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WorkKind {
    pub layer: &'static str,
    pub variant: &'static str,
}

impl Display for WorkKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}::{}", self.layer, self.variant)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AllocationCount {
    pub allocations: u64,
    pub bytes: u64,
}

impl AllocationCount {
    #[must_use]
    pub fn current() -> Self {
        AllocationCount {
            allocations: ALLOCATION_COUNT.load(Ordering::Relaxed),
            bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
        }
    }

    #[must_use]
    pub fn since(self, earlier: AllocationCount) -> AllocationCount {
        AllocationCount {
            allocations: self
                .allocations
                .checked_sub(earlier.allocations)
                .expect("the allocation counter only grows"),
            bytes: self
                .bytes
                .checked_sub(earlier.bytes)
                .expect("the allocated byte counter only grows"),
        }
    }
}

pub fn note_allocation(bytes: usize) {
    ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
    ALLOCATED_BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorkTotals {
    pub runs: u64,
    pub busy: Duration,
    pub allocations: AllocationCount,
}

impl WorkTotals {
    fn add(&mut self, measured: MeasuredWork) {
        self.runs += 1;
        self.busy += measured.busy;
        self.allocations.allocations += measured.allocations.allocations;
        self.allocations.bytes += measured.allocations.bytes;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollOutcome {
    Delivered,
    Pending,
    Finished,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SourcePolls {
    pub polls: WorkTotals,
    pub deliveries: u64,
}

#[derive(Debug, Default)]
pub struct LoadTally {
    pub updates: HashMap<WorkKind, WorkTotals>,
    pub views: WorkTotals,
    pub source_polls: HashMap<&'static str, SourcePolls>,
}

#[derive(Debug, Clone, Copy)]
pub struct MeasuredWork {
    pub busy: Duration,
    pub allocations: AllocationCount,
}

pub struct WorkMeasurement {
    started: Instant,
    allocations_before: AllocationCount,
}

impl WorkMeasurement {
    #[must_use]
    pub fn start() -> Self {
        WorkMeasurement {
            started: Clock::real().instant,
            allocations_before: AllocationCount::current(),
        }
    }

    #[must_use]
    pub fn finish(self) -> MeasuredWork {
        MeasuredWork {
            busy: Clock::real()
                .instant
                .checked_duration_since(self.started)
                .expect("the monotonic clock does not run backwards"),
            allocations: AllocationCount::current().since(self.allocations_before),
        }
    }
}

fn with_tally(record: impl FnOnce(&mut LoadTally)) {
    record(
        &mut LOAD_TALLY
            .lock()
            .expect("no thread panicked while recording load"),
    );
}

pub fn record_update(kind: WorkKind, measured: MeasuredWork) {
    with_tally(|tally| tally.updates.entry(kind).or_default().add(measured));
}

pub fn record_view(measured: MeasuredWork) {
    with_tally(|tally| tally.views.add(measured));
}

pub fn record_source_poll(source_name: &'static str, measured: MeasuredWork, outcome: PollOutcome) {
    with_tally(|tally| {
        let source = tally.source_polls.entry(source_name).or_default();
        source.polls.add(measured);
        if outcome == PollOutcome::Delivered {
            source.deliveries += 1;
        }
    });
}

#[must_use]
pub fn take_load_tally() -> LoadTally {
    mem::take(
        &mut LOAD_TALLY
            .lock()
            .expect("no thread panicked while recording load"),
    )
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{AllocationCount, MeasuredWork, WorkKind, WorkTotals};

    #[test]
    fn totals_add_each_measured_run() {
        let mut totals = WorkTotals::default();
        let measured = MeasuredWork {
            busy: Duration::from_millis(3),
            allocations: AllocationCount {
                allocations: 2,
                bytes: 64,
            },
        };
        totals.add(measured);
        totals.add(measured);
        assert_eq!(totals.runs, 2);
        assert_eq!(totals.busy, Duration::from_millis(6));
        assert_eq!(totals.allocations.bytes, 128);
    }

    #[test]
    fn work_kinds_name_their_layer_and_variant() {
        let kind = WorkKind {
            layer: "Message",
            variant: "Weather",
        };
        assert_eq!(kind.to_string(), "Message::Weather");
    }
}
