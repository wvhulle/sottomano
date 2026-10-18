use std::{io, thread, time::Duration};

use iced::futures::{Stream, stream};
use perf_event::{Builder, Counter, events::Dynamic};
use tokio::task::spawn_blocking;
use uom::si::{f64::Ratio, ratio::ratio};

use super::{Error, cpu_topology::cpulist, read_trimmed};
use crate::{
    data::source::{
        DeliveryCadence, ReadingSource, SourceDelivery, blocking_poll, polled_delivery_stream,
    },
    stream_operators::StreamOperators,
};

const L3_PMU: &str = "amd_l3";
const L3_PMU_CPUMASK: &str = "/sys/bus/event_source/devices/amd_l3/cpumask";
const L3_LOOKUP_STATE_EVENT: u64 = 0x04;
const ALL_L3_LOOKUPS_UMASK: u64 = 0xff;
const L3_MISS_UMASK: u64 = 0x01;
const SAMPLE_GAP: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, PartialEq)]
pub struct L3CacheTraffic {
    pub caches: Vec<SharedCacheTraffic>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SharedCacheTraffic {
    pub counted_on_cpu: u32,
    pub lookups_per_second: f64,
    pub hit_rate: Ratio,
}

impl ReadingSource for L3CacheTraffic {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "l3 cache traffic";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(3));

    fn delivery_stream(inputs: ()) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        let countable = async {
            spawn_blocking(|| read_l3_cache_traffic().is_ok())
                .await
                .unwrap_or(false)
        };
        stream::once(countable).while_held(
            move || polled_delivery_stream(inputs, Self::poll),
            SourceDelivery::Absent,
        )
    }
}

impl L3CacheTraffic {
    async fn poll(_key: ()) -> Result<Self, Error> {
        blocking_poll(read_l3_cache_traffic).await
    }
}

struct LookupCounters {
    cpu: u32,
    lookups: Counter,
    misses: Counter,
}

impl LookupCounters {
    fn open(cpu: u32) -> io::Result<Self> {
        let counter = |umask: u64| -> io::Result<Counter> {
            let event = Dynamic::builder(L3_PMU)?
                .field("event", L3_LOOKUP_STATE_EVENT)
                .and_then(|builder| builder.field("umask", umask))
                .map_err(io::Error::other)?
                .build()
                .map_err(io::Error::other)?;
            let mut counter = Builder::new(event)
                .one_cpu(cpu as usize)
                .any_pid()
                .include_kernel()
                .include_hv()
                .build()?;
            counter.enable()?;
            Ok(counter)
        };
        Ok(LookupCounters {
            cpu,
            lookups: counter(ALL_L3_LOOKUPS_UMASK)?,
            misses: counter(L3_MISS_UMASK)?,
        })
    }

    fn traffic(mut self, gap: Duration) -> io::Result<SharedCacheTraffic> {
        let lookups = self.lookups.read()? as f64;
        let misses = self.misses.read()? as f64;
        Ok(SharedCacheTraffic {
            counted_on_cpu: self.cpu,
            lookups_per_second: lookups / gap.as_secs_f64(),
            hit_rate: Ratio::new::<ratio>(if lookups > 0.0 {
                1.0 - misses / lookups
            } else {
                1.0
            }),
        })
    }
}

fn read_l3_cache_traffic() -> Result<L3CacheTraffic, Error> {
    let counting_cpus =
        read_trimmed(L3_PMU_CPUMASK).ok_or_else(|| Error::Unreadable(L3_PMU_CPUMASK.to_owned()))?;
    let counters = cpulist(&counting_cpus)
        .map(LookupCounters::open)
        .collect::<io::Result<Vec<_>>>()
        .map_err(|failure| Error::Unreadable(format!("{L3_PMU}: {failure}")))?;
    thread::sleep(SAMPLE_GAP);
    let caches = counters
        .into_iter()
        .map(|counters| counters.traffic(SAMPLE_GAP))
        .collect::<io::Result<Vec<_>>>()
        .map_err(|failure| Error::Unreadable(format!("{L3_PMU}: {failure}")))?;
    Ok(L3CacheTraffic { caches })
}
