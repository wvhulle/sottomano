use std::{
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use futures::Stream;
use uom::si::{f64::Power, power::watt};

use super::{Error, list_dir, read_i64, read_trimmed};
use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery, blocking_poll, polled_delivery_stream,
};

const POWERCAP: &str = "/sys/class/powercap";
const SAMPLE_GAP: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, PartialEq)]
pub struct CpuPower {
    pub zones: Vec<PowerZone>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PowerZone {
    pub name: String,
    pub draw: Power,
}

impl ReadingSource for CpuPower {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "cpu power";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(3));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl CpuPower {
    async fn poll(_key: ()) -> Result<Self, Error> {
        blocking_poll(|| Ok(read_cpu_power())).await
    }
}

struct EnergyCounter {
    zone: PathBuf,
    name: String,
    microjoules: i64,
    wraps_at: i64,
}

impl EnergyCounter {
    fn read_all() -> Vec<Self> {
        list_dir(POWERCAP)
            .into_iter()
            .filter(|zone| {
                zone.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("intel-rapl:"))
            })
            .filter_map(|zone| Self::read(&zone))
            .collect()
    }

    fn read(zone: &Path) -> Option<Self> {
        Some(EnergyCounter {
            zone: zone.to_path_buf(),
            name: read_trimmed(zone.join("name"))?,
            microjoules: read_i64(zone.join("energy_uj"))?,
            wraps_at: read_i64(zone.join("max_energy_range_uj"))?,
        })
    }

    fn draw_since(&self, earlier: &EnergyCounter, gap: Duration) -> Power {
        let spent = (self.microjoules - earlier.microjoules).rem_euclid(self.wraps_at + 1);
        Power::new::<watt>(spent as f64 / 1_000_000.0 / gap.as_secs_f64())
    }
}

fn read_cpu_power() -> CpuPower {
    let earlier = EnergyCounter::read_all();
    thread::sleep(SAMPLE_GAP);
    let later = EnergyCounter::read_all();
    let mut zones: Vec<PowerZone> = later
        .iter()
        .filter_map(|counter| {
            let before = earlier.iter().find(|before| before.zone == counter.zone)?;
            Some(PowerZone {
                name: counter.name.clone(),
                draw: counter.draw_since(before, SAMPLE_GAP),
            })
        })
        .collect();
    zones.sort_by(|first, second| first.name.cmp(&second.name));
    CpuPower { zones }
}
