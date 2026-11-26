use sottomano_desktop_sources::network_connectivity::network_manager_proxies::RadioPower;

use super::{WifiAdaptersGroup, WifiAdaptersInteraction};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator, indicator_state::IndicatorState,
        toggle_control::SwitchPosition,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::value_cells::LabeledValue,
};

pub struct AllWifiRadiosSwitch {
    pub radio_power: RadioPower,
}

impl AllWifiRadiosSwitch {
    #[must_use]
    pub fn press_does(&self) -> &'static str {
        match self.radio_power {
            RadioPower::On => "turn all Wi-Fi radios off",
            RadioPower::Off => "turn all Wi-Fi radios on",
        }
    }

    pub fn indicator<'indicator>(self) -> GroupIndicator<'indicator> {
        let glyph = match self.radio_power {
            RadioPower::On => Phosphor::WifiHigh(Weight::Regular).into(),
            RadioPower::Off => Phosphor::WifiSlash(Weight::Regular).into(),
        };
        GroupIndicator::glyph_only(
            "All Wi-Fi radios",
            glyph,
            IndicatorState::from(SwitchPosition::from(self.radio_power)),
        )
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Radio power",
            dictionary_table([LabeledValue::new(
                "All Wi-Fi radios",
                self.radio_power.label(),
            )]),
        )]
    }
}

impl WifiAdaptersGroup {
    pub(super) fn all_radios_row(&self, radio_power: RadioPower) -> GroupIndicator<'_> {
        let radios_switch = AllWifiRadiosSwitch { radio_power };
        let radios_press_does = radios_switch.press_does();
        radios_switch
            .indicator()
            .on_switch_press(
                radio_power,
                radios_press_does,
                WifiAdaptersInteraction::ToggleAllRadios,
            )
            .with_hover_card(Box::new(move |_| {
                self.wifi
                    .hover_card_with(AllWifiRadiosSwitch { radio_power }.hover_card())
            }))
    }
}
