use std::time::Duration;

use futures::Stream;
use procfs::{Current, MemoryPressure, PressureRecord};
use uom::si::{f64::Ratio, ratio::percent};

use crate::{
    SnapshotDelivery, SnapshotSource, delivery_cadence_drivers::PeriodicPolling,
    system_protocol_clients::sysfs_attribute_reads::Error,
};

const MEMORY_PRESSURE: &str = "/proc/pressure/memory";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StallAverages {
    pub last_10_seconds: Ratio,
    pub last_minute: Ratio,
    pub last_5_minutes: Ratio,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MemoryStallPressure {
    pub some: StallAverages,
    pub full: StallAverages,
}

impl From<&PressureRecord> for StallAverages {
    fn from(record: &PressureRecord) -> Self {
        Self {
            last_10_seconds: Ratio::new::<percent>(f64::from(record.avg10)),
            last_minute: Ratio::new::<percent>(f64::from(record.avg60)),
            last_5_minutes: Ratio::new::<percent>(f64::from(record.avg300)),
        }
    }
}

impl SnapshotSource for MemoryStallPressure {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "RAM pressure";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(Duration::from_secs(2))
            .blocking_poll_deliveries(read_memory_stall_pressure)
    }
}

fn read_memory_stall_pressure() -> Result<MemoryStallPressure, Error> {
    let pressure =
        MemoryPressure::current().map_err(|_| Error::Unreadable(MEMORY_PRESSURE.to_owned()))?;
    Ok(MemoryStallPressure {
        some: StallAverages::from(&pressure.some),
        full: StallAverages::from(&pressure.full),
    })
}
