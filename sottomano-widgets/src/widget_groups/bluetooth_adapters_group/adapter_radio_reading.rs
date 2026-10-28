use std::iter::once;

use chrono::Locale;
use sottomano_sources::{
    network_connectivity::bluez_bluetooth::{AdapterHardware, AdapterPower, BluetoothAdapter},
    systemd_units_journal::journal_device_kernel_errors::DeviceKernelError,
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        input_report_sections::KernelErrorListing,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterNaming {
    SoleAdapter,
    OneOfSeveral { label: String },
}

pub struct AdapterRadioReading<'a> {
    pub adapter: &'a BluetoothAdapter,
    pub naming: AdapterNaming,
    pub kernel_errors: Vec<&'a DeviceKernelError>,
}

pub struct AdapterPowerSwitch<'a> {
    pub adapter: &'a BluetoothAdapter,
}

fn power_description(power: &AdapterPower) -> &'static str {
    match power {
        AdapterPower::Off => "off",
        AdapterPower::SwitchingOn => "turning on",
        AdapterPower::SwitchingOff => "turning off",
        AdapterPower::On(_) => "on",
    }
}

impl AdapterRadioReading<'_> {
    #[must_use]
    pub fn kernel_device_of(controller_name: &str) -> String {
        format!("+bluetooth:{controller_name}")
    }

    fn overview_section(&self) -> Section {
        let powered_counts = match &self.adapter.power {
            AdapterPower::On(powered) => vec![
                KeyValue::new("Connected", Quantity::count(powered.connected.len())),
                KeyValue::new("Paired", Quantity::count(powered.paired_count)),
            ],
            AdapterPower::Off | AdapterPower::SwitchingOn | AdapterPower::SwitchingOff => {
                Vec::new()
            }
        };
        Section::dictionary(
            once(KeyValue::new(
                "Power",
                power_description(&self.adapter.power),
            ))
            .chain(powered_counts),
        )
        .titled(format!("Bluetooth {}", self.adapter.controller_name))
    }

    fn hardware_section(&self) -> Option<Section> {
        match &self.adapter.hardware {
            AdapterHardware::Identified(hardware) => Some(Section::from(hardware)),
            AdapterHardware::Unidentified => None,
        }
    }
}

impl BarWidget for AdapterRadioReading<'_> {
    fn name(&self) -> &'static str {
        "Bluetooth"
    }

    fn icon(&self) -> Glyph {
        match self.adapter.power {
            AdapterPower::On(_) | AdapterPower::SwitchingOn => {
                Phosphor::Bluetooth(Weight::Regular).into()
            }
            AdapterPower::Off | AdapterPower::SwitchingOff => {
                Phosphor::BluetoothSlash(Weight::Regular).into()
            }
        }
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        match &self.naming {
            AdapterNaming::SoleAdapter => None,
            AdapterNaming::OneOfSeveral { label } => Some(label.clone()),
        }
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.adapter.power.connected_devices().len())
            .ok()
            .filter(|count| *count > 0)
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        match self.adapter.power {
            AdapterPower::SwitchingOn | AdapterPower::SwitchingOff => WidgetState::Pending,
            AdapterPower::Off => WidgetState::Off,
            AdapterPower::On(_) if !self.kernel_errors.is_empty() => WidgetState::Warning,
            AdapterPower::On(_) => WidgetState::Idle,
        }
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections(
            [
                Some(self.overview_section()),
                KernelErrorListing {
                    kernel_errors: &self.kernel_errors,
                }
                .section(),
                self.hardware_section(),
            ]
            .into_iter()
            .flatten(),
        )
    }
}

impl AdapterPowerSwitch<'_> {
    #[must_use]
    pub fn press_does(&self) -> &'static str {
        match self.adapter.power {
            AdapterPower::On(_) | AdapterPower::SwitchingOn => "turn this Bluetooth adapter off",
            AdapterPower::Off | AdapterPower::SwitchingOff => "turn this Bluetooth adapter on",
        }
    }
}

impl BarWidget for AdapterPowerSwitch<'_> {
    fn name(&self) -> &'static str {
        "Bluetooth power"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Power(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(power_description(&self.adapter.power).to_owned())
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        match self.adapter.power {
            AdapterPower::SwitchingOn | AdapterPower::SwitchingOff => WidgetState::Pending,
            AdapterPower::Off => WidgetState::Off,
            AdapterPower::On(_) => WidgetState::On,
        }
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([Section::dictionary([
            KeyValue::new("Adapter", self.adapter.controller_name.as_str()),
            KeyValue::new("Power", power_description(&self.adapter.power)),
        ])])
    }
}
