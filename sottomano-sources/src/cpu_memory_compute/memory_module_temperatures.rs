use std::time::Duration;

use futures::Stream;
use uom::si::f64::ThermodynamicTemperature;

use super::hwmon_temperature_sensors::{HwmonChip, TemperatureSensor};
use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery, blocking_poll, polled_delivery_after_setup,
    system_protocol_clients::sysfs_attribute_reads::Error,
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

    fn delivery_stream((): ()) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_after_setup(
            blocking_poll(|| Ok(MemoryModuleSensor::discover())),
            Self::poll,
        )
    }
}

impl MemoryModuleTemperatures {
    async fn poll(sensors: Vec<MemoryModuleSensor>) -> Result<Self, Error> {
        blocking_poll(move || {
            Ok(MemoryModuleTemperatures {
                modules: sensors
                    .iter()
                    .filter_map(MemoryModuleSensor::read)
                    .collect(),
            })
        })
        .await
    }
}

#[derive(Debug, Clone)]
pub struct MemoryModuleSensor {
    bus_address: String,
    chip: HwmonChip,
    sensor: TemperatureSensor,
}

impl MemoryModuleSensor {
    fn discover() -> Vec<Self> {
        let mut sensors: Vec<Self> = HwmonChip::all()
            .into_iter()
            .filter(|chip| MEMORY_MODULE_SENSOR_CHIPS.contains(&chip.name.as_str()))
            .filter_map(|chip| {
                Some(MemoryModuleSensor {
                    bus_address: chip.device_name()?,
                    sensor: chip.temperature_sensors().into_iter().next()?,
                    chip,
                })
            })
            .collect();
        sensors.sort_by(|first, second| first.bus_address.cmp(&second.bus_address));
        sensors
    }

    fn read(&self) -> Option<MemoryModuleTemperature> {
        Some(MemoryModuleTemperature {
            bus_address: self.bus_address.clone(),
            temperature: self.chip.temperature(&self.sensor)?,
        })
    }
}
