pub mod core_kind_sections;
pub mod cpu_clock_label;
pub mod cpu_load_gauge;
pub mod cpu_temperature_label;
pub mod memory_usage_gauge;

use iced::{Subscription, Task};

use crate::{
    config::{Config, widget_groups::temperature_sensors::SensorConfig},
    data::source::{
        Source,
        hardware::HostHardware,
        sysfs_sensors::{
            self,
            cpu_clock::CpuClock,
            cpu_topology::CpuTopology,
            cpu_usage::{CpuCoreLoads, CpuUsage},
            memory::Memory,
            temperature::{CpuCoreTemperatures, Temperature},
        },
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::{polled, polling},
    widgets::{
        continuous_reading::ContinuousReading,
        hardware_section::HardwareEntries,
        on_hover_reading::{HoverFetched, OnHoverReading},
        tooltip_body::Tooltip,
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputeTarget {
    Cpu,
    CpuClock,
    Memory,
    Temperature,
}

impl From<ComputeTarget> for HoverTarget {
    fn from(target: ComputeTarget) -> Self {
        HoverTarget::Compute(target)
    }
}

#[derive(Default)]
pub struct ComputeGroup {
    cpu: ContinuousReading<CpuUsage>,
    memory: ContinuousReading<Memory>,
    clock: ContinuousReading<CpuClock>,
    temperature: ContinuousReading<Temperature>,
    sensors: bool,
    hardware: HostHardware,
    topology: CpuTopology,
    core_loads: OnHoverReading<CpuCoreLoads>,
    core_temperatures: OnHoverReading<CpuCoreTemperatures>,
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum ComputeMessage {
    Sensors(bool),
    Identified(HostHardware),
    TopologyDetected(CpuTopology),
    Cpu(Result<CpuUsage, sysfs_sensors::Error>),
    Memory(Result<Memory, sysfs_sensors::Error>),
    Clock(Result<CpuClock, sysfs_sensors::Error>),
    Temperature(Result<Temperature, sysfs_sensors::Error>),
    CoreLoadsFetched(HoverFetched<CpuCoreLoads>),
    CoreTemperaturesFetched(HoverFetched<CpuCoreTemperatures>),
    Expansion(ExpansionMessage),
}

impl ComputeGroup {
    pub fn probe(sensor: &SensorConfig) -> Task<ComputeMessage> {
        let sensor = sensor.clone();
        Task::batch([
            Task::future(
                async move { ComputeMessage::Sensors(Temperature::available(&sensor).await) },
            ),
            Task::future(HostHardware::identify()).map(ComputeMessage::Identified),
            Task::done(ComputeMessage::TopologyDetected(CpuTopology::detect())),
        ])
    }

    pub fn hover(
        &mut self,
        hovered: Option<&HoverTarget>,
        config: &Config,
    ) -> Task<ComputeMessage> {
        self.core_loads.stop();
        self.core_temperatures.stop();
        match hovered {
            Some(HoverTarget::Compute(ComputeTarget::Cpu)) => self.core_loads.fetch(
                polled::<CpuCoreLoads>(()),
                ComputeMessage::CoreLoadsFetched,
                config,
            ),
            Some(HoverTarget::Compute(ComputeTarget::Temperature)) => self.core_temperatures.fetch(
                polled::<CpuCoreTemperatures>(
                    config.widget_groups.temperature_sensors.cpu.overrides(),
                ),
                ComputeMessage::CoreTemperaturesFetched,
                config,
            ),
            _ => Task::none(),
        }
    }

    pub fn subscription(&self, sensor: &SensorConfig) -> Subscription<ComputeMessage> {
        let temperature = if self.sensors {
            polling::<Temperature>(sensor.clone()).map(ComputeMessage::Temperature)
        } else {
            Subscription::none()
        };
        Subscription::batch([
            polling::<CpuUsage>(()).map(ComputeMessage::Cpu),
            polling::<Memory>(()).map(ComputeMessage::Memory),
            polling::<CpuClock>(()).map(ComputeMessage::Clock),
            temperature,
        ])
    }

    pub fn update(&mut self, msg: ComputeMessage) -> Task<ComputeMessage> {
        match msg {
            ComputeMessage::Sensors(sensors) => self.sensors = sensors,
            ComputeMessage::Identified(hardware) => self.hardware = hardware,
            ComputeMessage::TopologyDetected(topology) => self.topology = topology,
            ComputeMessage::CoreLoadsFetched(fetched) => self.core_loads.receive(fetched),
            ComputeMessage::CoreTemperaturesFetched(fetched) => {
                self.core_temperatures.receive(fetched);
            }
            ComputeMessage::Cpu(reading) => self.cpu.receive(reading),
            ComputeMessage::Memory(reading) => self.memory.receive(reading),
            ComputeMessage::Clock(reading) => self.clock.receive(reading),
            ComputeMessage::Temperature(reading) => self.temperature.receive(reading),
            ComputeMessage::Expansion(m) => {
                self.expansion.update(m);
            }
        }
        Task::none()
    }

    pub fn group_content(&self) -> GroupContent<'_, ComputeMessage> {
        GroupContent::new(
            &self.expansion,
            ComputeMessage::Expansion,
            [
                self.cpu.group_item(ComputeTarget::Cpu),
                self.memory.group_item(ComputeTarget::Memory),
                self.clock.group_item(ComputeTarget::CpuClock),
                self.temperature.group_item(ComputeTarget::Temperature),
            ]
            .into_iter()
            .flatten(),
        )
    }

    pub fn tooltip_for(&self, target: ComputeTarget) -> Option<Tooltip> {
        match target {
            ComputeTarget::Cpu => self.cpu.tooltip_with_sections(
                &[
                    vec![
                        self.core_loads
                            .section(|loads| loads.sections(&self.topology)),
                    ],
                    self.hardware.cpu.hardware_sections().to_vec(),
                ]
                .concat(),
            ),
            ComputeTarget::Memory => self
                .memory
                .tooltip_with_sections(&[self.hardware.memory.hardware_section()]),
            ComputeTarget::CpuClock => self.clock.tooltip_with_sections(
                &self
                    .clock
                    .get()
                    .map(|clock| clock.core_sections(&self.topology))
                    .unwrap_or_default(),
            ),
            ComputeTarget::Temperature => self.temperature.tooltip_with_sections(&[self
                .core_temperatures
                .section(|cores| cores.sections(&self.topology))]),
        }
    }
}
