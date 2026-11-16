use std::iter::once;

use sottomano_desktop_sources::{
    network_connectivity::bluez_bluetooth::{AdapterPower, BluetoothAdapter},
    systemd_units_journal::journal_device_kernel_errors::DeviceKernelError,
};

use crate::{
    group_indicators::{
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
        reading_row::ReadingRow,
        toggle_control::SwitchPosition,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        input_report_sections::KernelErrorListing,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterNaming {
    SoleAdapter,
    OneOfSeveral { label: String },
}

pub struct AdapterRadioIndicator<'a> {
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

impl AdapterRadioIndicator<'_> {
    #[must_use]
    pub fn kernel_device_of(controller_name: &str) -> String {
        format!("+bluetooth:{controller_name}")
    }

    fn overview_section(&self) -> Section {
        let connected_count = match &self.adapter.power {
            AdapterPower::On(powered) => Some(LabeledValue::new(
                "Connected",
                Quantity::count(powered.connected.len()),
            )),
            AdapterPower::Off | AdapterPower::SwitchingOn | AdapterPower::SwitchingOff => None,
        };
        Section::new(
            format!("Bluetooth {}", self.adapter.controller_name),
            dictionary_table(
                once(LabeledValue::new(
                    "Power",
                    power_description(&self.adapter.power),
                ))
                .chain(connected_count),
            ),
        )
    }
}

impl<'a> AdapterRadioIndicator<'a> {
    #[must_use]
    pub fn reading_row(self) -> ReadingRow<'a> {
        let glyph = match self.adapter.power {
            AdapterPower::On(_) | AdapterPower::SwitchingOn => {
                Phosphor::Bluetooth(Weight::Regular).into()
            }
            AdapterPower::Off | AdapterPower::SwitchingOff => {
                Phosphor::BluetoothSlash(Weight::Regular).into()
            }
        };
        let state = match self.adapter.power {
            AdapterPower::SwitchingOn | AdapterPower::SwitchingOff => IndicatorState::Pending,
            AdapterPower::Off => IndicatorState::Off,
            AdapterPower::On(_) if !self.kernel_errors.is_empty() => IndicatorState::Warning,
            AdapterPower::On(_) => IndicatorState::Idle,
        };
        let label = match &self.naming {
            AdapterNaming::OneOfSeveral { label } if !label.is_empty() => {
                ReadoutLabel::Labeled(label.clone())
            }
            AdapterNaming::SoleAdapter | AdapterNaming::OneOfSeveral { .. } => {
                ReadoutLabel::Unlabeled
            }
        };
        let badge = match self.adapter.power.connected_devices().len() {
            0 => ReadoutBadge::Unbadged,
            connected_count => ReadoutBadge::Counted(connected_count),
        };
        ReadingRow {
            readout: IndicatorReadout {
                label,
                gauge: ReadoutGauge::Ungauged,
                badge,
            },
            ..ReadingRow::glyph_only(
                "Bluetooth adapter",
                glyph,
                state,
                Box::new(move |_| self.hover_card()),
            )
        }
    }

    #[must_use]
    pub fn hover_card(&self) -> Vec<Section> {
        [
            Some(self.overview_section()),
            KernelErrorListing {
                kernel_errors: &self.kernel_errors,
            }
            .section(),
        ]
        .into_iter()
        .flatten()
        .collect()
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

impl From<&AdapterPower> for SwitchPosition {
    fn from(adapter_power: &AdapterPower) -> Self {
        match adapter_power {
            AdapterPower::On(_) | AdapterPower::SwitchingOn => SwitchPosition::On,
            AdapterPower::Off | AdapterPower::SwitchingOff => SwitchPosition::Off,
        }
    }
}

impl<'a> AdapterPowerSwitch<'a> {
    #[must_use]
    pub fn reading_row(self) -> ReadingRow<'a> {
        let state = match self.adapter.power {
            AdapterPower::SwitchingOn | AdapterPower::SwitchingOff => IndicatorState::Pending,
            AdapterPower::Off => IndicatorState::Off,
            AdapterPower::On(_) => IndicatorState::On,
        };
        ReadingRow {
            readout: IndicatorReadout {
                label: ReadoutLabel::Labeled(power_description(&self.adapter.power).to_owned()),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                "Bluetooth power",
                Phosphor::Power(Weight::Regular).into(),
                state,
                Box::new(move |_| self.hover_card()),
            )
        }
    }

    #[must_use]
    pub fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Status",
            dictionary_table([
                LabeledValue::new("Adapter", self.adapter.controller_name.as_str()),
                LabeledValue::new("Power", power_description(&self.adapter.power)),
            ]),
        )]
    }
}
