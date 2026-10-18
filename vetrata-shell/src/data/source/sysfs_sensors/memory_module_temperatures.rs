use std::time::Duration;

use iced::futures::Stream;
use uom::si::f64::ThermodynamicTemperature;

use super::{Error, hwmon::HwmonChip};
use crate::data::source::{
    DeliveryCadence, ReadingSource, SourceDelivery, blocking_poll, polled_delivery_stream,
};

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

impl ReadingSource for MemoryModuleTemperatures {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "memory module temperatures";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(3));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl MemoryModuleTemperatures {
    async fn poll(_key: ()) -> Result<Self, Error> {
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
