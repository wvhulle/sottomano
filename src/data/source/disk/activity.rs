use std::{
    fs,
    future::ready,
    marker::PhantomData,
    time::{Duration, Instant},
};

use iced::futures::{Stream, StreamExt};
use uom::si::{
    f64::{Information, InformationRate, Ratio, Time},
    information::byte,
    ratio::ratio,
    time::{millisecond, second},
};

use super::{Error, mount::RootMount};
use crate::message_subscriptions::ticked;

const SAMPLE_INTERVAL: Duration = Duration::from_secs(3);
const SECTOR_BYTES: f64 = 512.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Reading;

#[derive(Debug, Clone, PartialEq)]
pub struct Writing;

#[derive(Debug, Clone, PartialEq)]
pub struct DiskTransfer<D> {
    pub disk: String,
    pub rate: InformationRate,
    pub disk_busy: Ratio,
    direction: PhantomData<D>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiskActivity {
    pub read: DiskTransfer<Reading>,
    pub write: DiskTransfer<Writing>,
}

struct CounterSample {
    disk: String,
    counters: BlockCounters,
    at: Instant,
}

impl CounterSample {
    fn take() -> Result<Self, Error> {
        let disk = RootMount::current()?.device()?.whole_disk();
        let stat = disk.sysfs_dir().join("stat");
        let line =
            fs::read_to_string(&stat).map_err(|_| Error::Unreadable(stat.display().to_string()))?;
        Ok(CounterSample {
            counters: BlockCounters::parse(&line)
                .ok_or_else(|| Error::Unparsable(stat.display().to_string()))?,
            disk: disk.name,
            at: Instant::now(),
        })
    }
}

pub fn readings() -> impl Stream<Item = Result<DiskActivity, Error>> {
    ticked(SAMPLE_INTERVAL, || ready(CounterSample::take()))
        .scan(None::<CounterSample>, |earlier, sample| {
            let activity = match sample {
                Err(failure) => {
                    *earlier = None;
                    Some(Err(failure))
                }
                Ok(later) => {
                    let activity = earlier.as_ref().map(|earlier| {
                        Ok(later.counters.activity_since(
                            &earlier.counters,
                            later.at - earlier.at,
                            later.disk.clone(),
                        ))
                    });
                    *earlier = Some(later);
                    activity
                }
            };
            ready(Some(activity))
        })
        .filter_map(ready)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BlockCounters {
    sectors_read: u64,
    sectors_written: u64,
    busy_milliseconds: u64,
}

impl BlockCounters {
    fn parse(line: &str) -> Option<Self> {
        let fields: Vec<u64> = line
            .split_whitespace()
            .map(str::parse)
            .collect::<Result<_, _>>()
            .ok()?;
        Some(BlockCounters {
            sectors_read: *fields.get(2)?,
            sectors_written: *fields.get(6)?,
            busy_milliseconds: *fields.get(9)?,
        })
    }

    fn activity_since(&self, earlier: &Self, elapsed: Duration, disk: String) -> DiskActivity {
        let elapsed = Time::new::<second>(elapsed.as_secs_f64());
        let rate = |now: u64, was: u64| -> InformationRate {
            let sectors = now.saturating_sub(was) as f64;
            (Information::new::<byte>(sectors * SECTOR_BYTES) / elapsed).into()
        };
        let busy = Time::new::<millisecond>(
            self.busy_milliseconds
                .saturating_sub(earlier.busy_milliseconds) as f64,
        );
        let disk_busy = (busy / elapsed).min(Ratio::new::<ratio>(1.0));
        DiskActivity {
            read: DiskTransfer::new(
                disk.clone(),
                rate(self.sectors_read, earlier.sectors_read),
                disk_busy,
            ),
            write: DiskTransfer::new(
                disk,
                rate(self.sectors_written, earlier.sectors_written),
                disk_busy,
            ),
        }
    }
}

impl<D> DiskTransfer<D> {
    fn new(disk: String, rate: InformationRate, disk_busy: Ratio) -> Self {
        DiskTransfer {
            disk,
            rate,
            disk_busy,
            direction: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use uom::si::{information_rate::kibibyte_per_second, ratio::percent};

    use super::*;

    const IDLE: &str =
        "  150258    45414 14929119    31977   102277    26257  7734678    48886        0    \
         18687    81766";
    const AFTER_ONE_SECOND: &str = "  150300    45414 14931167    31990   102300    26257  \
                                    7738774    48900        0    18937    81800";

    #[test]
    fn block_stat_fields_are_located() {
        let counters = BlockCounters::parse(IDLE).unwrap();
        assert_eq!(counters.sectors_read, 14_929_119);
        assert_eq!(counters.sectors_written, 7_734_678);
        assert_eq!(counters.busy_milliseconds, 18_687);
    }

    #[test]
    fn rates_and_busy_fraction_follow_from_two_samples() {
        let earlier = BlockCounters::parse(IDLE).unwrap();
        let later = BlockCounters::parse(AFTER_ONE_SECOND).unwrap();
        let activity = later.activity_since(&earlier, Duration::from_secs(1), "nvme0n1".into());
        assert!((activity.read.rate.get::<kibibyte_per_second>() - 1024.0).abs() < 1e-6);
        assert!((activity.write.rate.get::<kibibyte_per_second>() - 2048.0).abs() < 1e-6);
        assert!((activity.write.disk_busy.get::<percent>() - 25.0).abs() < 1e-6);
    }
}
