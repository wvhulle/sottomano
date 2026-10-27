use std::time::Duration;

use futures::Stream;
use uom::si::f64::ThermodynamicTemperature;

use super::hwmon_temperature_sensors::{HwmonChip, TemperatureSensor};
use crate::{
    SnapshotDelivery, SnapshotSource, blocking_background_work::blocking_poll,
    delivery_cadence_drivers::PeriodicPolling,
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

impl SnapshotSource for MemoryModuleTemperatures {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "Module temperatures";

    fn delivery_stream((): ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(Duration::from_secs(3)).poll_deliveries_after_setup(
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
