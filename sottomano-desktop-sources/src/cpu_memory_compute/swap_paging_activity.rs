use std::{
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};

use bytesize::ByteSize;
use futures::Stream;
use procfs::{Current, Meminfo, page_size, vmstat};

use crate::{
    SnapshotDelivery, SnapshotSource, cpu_memory_compute::memory_usage::MEMINFO,
    delivery_cadence_drivers::PeriodicPolling,
    system_protocol_clients::sysfs_attribute_reads::Error,
};

const VMSTAT: &str = "/proc/vmstat";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Swap {
    NotConfigured,
    Configured { used: ByteSize, total: ByteSize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwapActivity {
    pub swap: Swap,
    pub paged_in_per_second: ByteSize,
    pub paged_out_per_second: ByteSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SwappedPageCounters {
    sampled_at: Instant,
    pages_in: u64,
    pages_out: u64,
}

impl SnapshotSource for SwapActivity {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "Swap";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let previous_counters = Arc::new(Mutex::new(None));
        PeriodicPolling::every(Duration::from_secs(2)).blocking_poll_deliveries(move || {
            let mut previous = previous_counters
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            read_swap_activity(&mut previous, Instant::now())
        })
    }
}

fn read_swap_activity(
    previous: &mut Option<SwappedPageCounters>,
    now: Instant,
) -> Result<SwapActivity, Error> {
    let info = Meminfo::current().map_err(|_| Error::Unreadable(MEMINFO.to_owned()))?;
    let swap = if info.swap_total == 0 {
        Swap::NotConfigured
    } else {
        Swap::Configured {
            used: ByteSize::b(info.swap_total.saturating_sub(info.swap_free)),
            total: ByteSize::b(info.swap_total),
        }
    };
    let counters = vmstat().map_err(|_| Error::Unreadable(VMSTAT.to_owned()))?;
    let page_counter = |field: &str| {
        counters
            .get(field)
            .and_then(|pages| u64::try_from(*pages).ok())
            .ok_or_else(|| Error::Unparsable(format!("{VMSTAT} field {field}")))
    };
    let current = SwappedPageCounters {
        sampled_at: now,
        pages_in: page_counter("pswpin")?,
        pages_out: page_counter("pswpout")?,
    };
    let (paged_in_per_second, paged_out_per_second) = previous
        .map_or((ByteSize::b(0), ByteSize::b(0)), |earlier| {
            paging_rates(earlier, current, page_size())
        });
    *previous = Some(current);
    Ok(SwapActivity {
        swap,
        paged_in_per_second,
        paged_out_per_second,
    })
}

fn paging_rates(
    earlier: SwappedPageCounters,
    later: SwappedPageCounters,
    page_bytes: u64,
) -> (ByteSize, ByteSize) {
    let elapsed_seconds = later
        .sampled_at
        .saturating_duration_since(earlier.sampled_at)
        .as_secs_f64();
    if elapsed_seconds <= 0.0 {
        return (ByteSize::b(0), ByteSize::b(0));
    }
    let bytes_per_second = |pages_before: u64, pages_after: u64| {
        let swapped_bytes = pages_after.saturating_sub(pages_before) * page_bytes;
        ByteSize::b((swapped_bytes as f64 / elapsed_seconds) as u64)
    };
    (
        bytes_per_second(earlier.pages_in, later.pages_in),
        bytes_per_second(earlier.pages_out, later.pages_out),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paging_rates_divide_page_deltas_by_elapsed_time() {
        let start = Instant::now();
        let earlier = SwappedPageCounters {
            sampled_at: start,
            pages_in: 1_000,
            pages_out: 500,
        };
        let later = SwappedPageCounters {
            sampled_at: start + Duration::from_secs(2),
            pages_in: 1_512,
            pages_out: 500,
        };
        assert_eq!(
            paging_rates(earlier, later, 4_096),
            (ByteSize::kib(1_024), ByteSize::b(0))
        );
    }

    #[test]
    fn counters_without_elapsed_time_report_no_paging() {
        let start = Instant::now();
        let counters = SwappedPageCounters {
            sampled_at: start,
            pages_in: 10,
            pages_out: 10,
        };
        assert_eq!(
            paging_rates(counters, counters, 4_096),
            (ByteSize::b(0), ByteSize::b(0))
        );
    }
}
