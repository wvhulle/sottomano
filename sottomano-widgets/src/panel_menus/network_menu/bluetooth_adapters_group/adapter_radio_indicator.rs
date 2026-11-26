use std::{iter::once, time::SystemTime};

use sottomano_desktop_sources::{
    network_connectivity::bluez_bluetooth::{AdapterPower, BluetoothAdapter},
    system_protocol_clients::journal_follow::unexpired_events,
    systemd_units_journal::journal_device_kernel_errors::DeviceKernelError,
};

use super::BluetoothAdaptersGroup;
use crate::{
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        input_report_sections::KernelErrorListing, localized_quantities::Quantity,
        value_cells::LabeledValue,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterNaming {
    SoleAdapter,
    OneOfSeveral { label: String },
}

pub struct AdapterRadioIndicator<'view> {
    pub adapter: &'view BluetoothAdapter,
    pub naming: AdapterNaming,
    pub kernel_errors: Vec<&'view DeviceKernelError>,
}

pub(super) fn power_description(power: &AdapterPower) -> &'static str {
    match power {
        AdapterPower::Off => "off",
        AdapterPower::SwitchingOn => "turning on",
        AdapterPower::SwitchingOff => "turning off",
        AdapterPower::On(_) => "on",
    }
}

impl AdapterRadioIndicator<'_> {
    #[must_use]
    pub fn controller_kernel_device(controller_name: &str) -> String {
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

impl<'view> AdapterRadioIndicator<'view> {
    #[must_use]
    pub fn indicator(self) -> GroupIndicator<'view> {
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
        GroupIndicator::glyph_only("Bluetooth adapter", glyph, state)
            .with_readout(IndicatorReadout {
                label,
                gauge: ReadoutGauge::Ungauged,
                badge,
            })
            .with_hover_card(Box::new(move |_| self.hover_card()))
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

impl BluetoothAdaptersGroup {
    pub(super) fn adapter_radio_row<'view>(
        &'view self,
        adapter: &'view BluetoothAdapter,
        clock: Clock,
    ) -> GroupIndicator<'view> {
        self.adapter_indicator(adapter, clock.wall)
            .indicator()
            .with_hover_card(Box::new(move |card_clock| {
                self.bluetooth.hover_card_with(
                    self.adapter_indicator(adapter, card_clock.wall)
                        .hover_card(),
                )
            }))
    }

    fn adapter_indicator<'view>(
        &'view self,
        adapter: &'view BluetoothAdapter,
        now: SystemTime,
    ) -> AdapterRadioIndicator<'view> {
        let kernel_device =
            AdapterRadioIndicator::controller_kernel_device(&adapter.controller_name);
        AdapterRadioIndicator {
            adapter,
            naming: self.adapter_naming(adapter),
            kernel_errors: unexpired_events(&self.kernel_errors, now)
                .filter(|error| error.kernel_device == kernel_device)
                .collect(),
        }
    }
}
