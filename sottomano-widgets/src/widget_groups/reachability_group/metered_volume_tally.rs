use std::{
    collections::BTreeMap,
    mem,
    time::{Duration, Instant},
};

use bytesize::ByteSize;
use chrono::NaiveDate;
use sottomano_desktop_sources::network_connectivity::interface_throughput::{
    InterfaceByteCounters, InterfaceThroughput,
};

use super::sqlite_metered_volume::{MeteredVolumeTotals, TransferTotals};

const SAVE_INTERVAL: Duration = Duration::from_mins(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StoredTotals {
    NotLoaded,
    Loading,
    Loaded {
        totals: MeteredVolumeTotals,
        last_saved_at: Instant,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TallyFollowUp {
    Nothing,
    LoadTotals,
    SaveUnsaved(TransferTotals),
}

#[derive(Debug)]
pub struct MeteredVolumeTally {
    previous_counters: BTreeMap<String, InterfaceByteCounters>,
    unsaved: TransferTotals,
    stored: StoredTotals,
}

impl Default for MeteredVolumeTally {
    fn default() -> Self {
        Self {
            previous_counters: BTreeMap::new(),
            unsaved: TransferTotals::default(),
            stored: StoredTotals::NotLoaded,
        }
    }
}

fn counter_growth(now: ByteSize, was: ByteSize) -> ByteSize {
    ByteSize::b(now.as_u64().saturating_sub(was.as_u64()))
}

fn transferred_between(
    previous_counters: &BTreeMap<String, InterfaceByteCounters>,
    current_counters: &BTreeMap<String, InterfaceByteCounters>,
) -> TransferTotals {
    current_counters
        .iter()
        .filter_map(|(interface, now)| Some((now, previous_counters.get(interface)?)))
        .map(|(now, was)| TransferTotals {
            received: counter_growth(now.received, was.received),
            sent: counter_growth(now.sent, was.sent),
        })
        .fold(TransferTotals::default(), |summed, transferred| {
            summed + transferred
        })
}

impl MeteredVolumeTally {
    pub fn observe(&mut self, throughput: &InterfaceThroughput, now: Instant) -> TallyFollowUp {
        let transferred =
            transferred_between(&self.previous_counters, &throughput.cumulative_counters);
        self.previous_counters
            .clone_from(&throughput.cumulative_counters);
        if !throughput.metered {
            return TallyFollowUp::Nothing;
        }
        self.unsaved = self.unsaved + transferred;
        match self.stored {
            StoredTotals::NotLoaded => {
                self.stored = StoredTotals::Loading;
                TallyFollowUp::LoadTotals
            }
            StoredTotals::Loaded {
                totals,
                last_saved_at,
            } if now.duration_since(last_saved_at) >= SAVE_INTERVAL
                && self.unsaved != TransferTotals::default() =>
            {
                let saving = mem::take(&mut self.unsaved);
                self.stored = StoredTotals::Loaded {
                    totals: totals.with_unsaved(saving),
                    last_saved_at: now,
                };
                TallyFollowUp::SaveUnsaved(saving)
            }
            StoredTotals::Loading | StoredTotals::Loaded { .. } => TallyFollowUp::Nothing,
        }
    }

    pub fn totals_loaded(
        &mut self,
        loaded: Option<MeteredVolumeTotals>,
        today: NaiveDate,
        now: Instant,
    ) {
        let last_saved_at = match self.stored {
            StoredTotals::Loaded { last_saved_at, .. } => last_saved_at,
            StoredTotals::NotLoaded | StoredTotals::Loading => now,
        };
        self.stored = StoredTotals::Loaded {
            totals: loaded.unwrap_or_else(|| MeteredVolumeTotals::starting(today)),
            last_saved_at,
        };
    }

    #[must_use]
    pub fn displayed_totals(&self) -> Option<MeteredVolumeTotals> {
        match self.stored {
            StoredTotals::Loaded { totals, .. } => Some(totals.with_unsaved(self.unsaved)),
            StoredTotals::NotLoaded | StoredTotals::Loading => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::network_connectivity::interface_throughput::{
        NetworkTrafficDirection, Throughput,
    };
    use uom::si::f64::InformationRate;

    use super::*;
    use crate::clock::Clock;

    fn idle_direction(direction: NetworkTrafficDirection) -> Throughput {
        Throughput {
            interfaces: vec!["wlan0".to_owned()],
            rate: InformationRate::default(),
            peak_rate: InformationRate::default(),
            direction,
        }
    }

    fn throughput_at(received: u64, sent: u64, metered: bool) -> InterfaceThroughput {
        InterfaceThroughput {
            down: idle_direction(NetworkTrafficDirection::Download),
            up: idle_direction(NetworkTrafficDirection::Upload),
            metered,
            cumulative_counters: BTreeMap::from([(
                "wlan0".to_owned(),
                InterfaceByteCounters {
                    received: ByteSize::b(received),
                    sent: ByteSize::b(sent),
                },
            )]),
        }
    }

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 28).unwrap()
    }

    fn loaded_tally(started_at: Instant) -> MeteredVolumeTally {
        let mut tally = MeteredVolumeTally::default();
        assert_eq!(
            tally.observe(&throughput_at(1_000, 100, true), started_at),
            TallyFollowUp::LoadTotals
        );
        tally.totals_loaded(None, today(), started_at);
        tally
    }

    #[test]
    fn growth_is_counted_while_metered() {
        let started_at = Clock::real().instant;
        let mut tally = loaded_tally(started_at);
        tally.observe(&throughput_at(1_500, 160, true), started_at);
        assert_eq!(
            tally.displayed_totals().unwrap().today,
            TransferTotals {
                received: ByteSize::b(500),
                sent: ByteSize::b(60),
            }
        );
    }

    #[test]
    fn unmetered_traffic_is_not_counted() {
        let started_at = Clock::real().instant;
        let mut tally = loaded_tally(started_at);
        tally.observe(&throughput_at(9_000, 900, false), started_at);
        tally.observe(&throughput_at(9_100, 910, true), started_at);
        assert_eq!(
            tally.displayed_totals().unwrap().today,
            TransferTotals {
                received: ByteSize::b(100),
                sent: ByteSize::b(10),
            }
        );
    }

    #[test]
    fn counter_reset_adds_nothing() {
        let started_at = Clock::real().instant;
        let mut tally = loaded_tally(started_at);
        tally.observe(&throughput_at(10, 1, true), started_at);
        assert_eq!(
            tally.displayed_totals().unwrap().today,
            TransferTotals::default()
        );
    }

    #[test]
    fn unsaved_growth_is_saved_after_the_interval_and_stays_displayed() {
        let started_at = Clock::real().instant;
        let mut tally = loaded_tally(started_at);
        let later = started_at + SAVE_INTERVAL;
        let saved = TransferTotals {
            received: ByteSize::b(200),
            sent: ByteSize::b(20),
        };
        assert_eq!(
            tally.observe(&throughput_at(1_200, 120, true), later),
            TallyFollowUp::SaveUnsaved(saved)
        );
        assert_eq!(tally.displayed_totals().unwrap().today, saved);
    }
}
