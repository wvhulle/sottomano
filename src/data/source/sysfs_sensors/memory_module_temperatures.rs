use std::time::Duration;

use uom::si::f64::ThermodynamicTemperature;

use super::{Error, hwmon::HwmonChip};
use crate::data::source::{PolledSource, Source, blocking_poll};

const MEMORY_MODULE_SENSOR_CHIPS: [&str; 2] = ["spd5118", "jc42"];

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryModuleTemperatures {
    pub modules: Vec<MemoryModuleTemperature>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryModuleTemperature {
    pub bus_address: String,
    pub temperature: ThermodynamicTemperature,
}

impl Source for MemoryModuleTemperatures {
    type Error = Error;
    type Key = ();

    const NAME: &'static str = "memory module temperatures";
}

impl PolledSource for MemoryModuleTemperatures {
    const INTERVAL: Duration = Duration::from_secs(3);

    async fn poll(_key: ()) -> Result<Self, Self::Error> {
        blocking_poll(|| Ok(read_memory_module_temperatures())).await
    }
}

fn read_memory_module_temperatures() -> MemoryModuleTemperatures {
    let mut modules: Vec<MemoryModuleTemperature> = HwmonChip::all()
        .into_iter()
        .filter(|chip| MEMORY_MODULE_SENSOR_CHIPS.contains(&chip.name.as_str()))
        .filter_map(|chip| {
            let sensor = chip.temperature_sensors().into_iter().next()?;
            Some(MemoryModuleTemperature {
                bus_address: chip.device_name()?,
                temperature: chip.temperature(&sensor)?,
            })
        })
        .collect();
    modules.sort_by(|first, second| first.bus_address.cmp(&second.bus_address));
    MemoryModuleTemperatures { modules }
}
