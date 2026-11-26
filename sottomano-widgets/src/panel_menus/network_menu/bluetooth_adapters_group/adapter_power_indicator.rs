use sottomano_desktop_sources::network_connectivity::bluez_bluetooth::{
    AdapterPower, BluetoothAdapter,
};

use super::{
    BluetoothAdaptersGroup, BluetoothInteraction, adapter_radio_indicator::power_description,
};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
        toggle_control::SwitchPosition,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::value_cells::LabeledValue,
};
pub struct AdapterPowerSwitch<'view> {
    pub adapter: &'view BluetoothAdapter,
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

impl<'view> AdapterPowerSwitch<'view> {
    #[must_use]
    pub fn indicator(self) -> GroupIndicator<'view> {
        let state = match self.adapter.power {
            AdapterPower::SwitchingOn | AdapterPower::SwitchingOff => IndicatorState::Pending,
            AdapterPower::Off => IndicatorState::Off,
            AdapterPower::On(_) => IndicatorState::On,
        };
        GroupIndicator::glyph_only(
            "Bluetooth power",
            Phosphor::Power(Weight::Regular).into(),
            state,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(power_description(&self.adapter.power).to_owned()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
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

impl BluetoothAdaptersGroup {
    pub(super) fn adapter_power_row<'view>(
        &'view self,
        adapter: &'view BluetoothAdapter,
    ) -> GroupIndicator<'view> {
        let power_switch = AdapterPowerSwitch { adapter };
        let power_press_does = power_switch.press_does();
        power_switch
            .indicator()
            .on_switch_press(
                &adapter.power,
                power_press_does,
                BluetoothInteraction::ToggleAdapterPower(adapter.object_path.clone()),
            )
            .with_hover_card(Box::new(move |_| {
                self.bluetooth
                    .hover_card_with(AdapterPowerSwitch { adapter }.hover_card())
            }))
    }
}
