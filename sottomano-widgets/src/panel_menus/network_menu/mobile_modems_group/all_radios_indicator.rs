use sottomano_desktop_sources::network_connectivity::network_manager_proxies::RadioPower;

use super::{MobileModemsGroup, MobileModemsInteraction};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator, indicator_state::IndicatorState,
        toggle_control::SwitchPosition,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::value_cells::LabeledValue,
};
pub struct AllMobileRadiosSwitch {
    pub radio_power: RadioPower,
}

impl AllMobileRadiosSwitch {
    #[must_use]
    pub fn press_does(&self) -> &'static str {
        match self.radio_power {
            RadioPower::On => "turn all mobile radios off",
            RadioPower::Off => "turn all mobile radios on",
        }
    }

    pub fn indicator<'indicator>(self) -> GroupIndicator<'indicator> {
        let glyph = match self.radio_power {
            RadioPower::On => Phosphor::CellSignalFull(Weight::Regular).into(),
            RadioPower::Off => Phosphor::CellSignalSlash(Weight::Regular).into(),
        };
        GroupIndicator::glyph_only(
            "All mobile radios",
            glyph,
            IndicatorState::from(SwitchPosition::from(self.radio_power)),
        )
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Status",
            dictionary_table([LabeledValue::new(
                "All mobile radios",
                self.radio_power.label(),
            )]),
        )]
    }
}

impl MobileModemsGroup {
    pub(super) fn all_radios_row(&self) -> GroupIndicator<'_> {
        let any_radio_power = self.any_radio_power();
        let radios_switch = AllMobileRadiosSwitch {
            radio_power: any_radio_power,
        };
        let radios_press_does = radios_switch.press_does();
        radios_switch
            .indicator()
            .on_switch_press(
                any_radio_power,
                radios_press_does,
                MobileModemsInteraction::ToggleAllRadios,
            )
            .with_hover_card(Box::new(move |_| {
                self.all_radios_hover_card(any_radio_power)
            }))
    }

    fn any_radio_power(&self) -> RadioPower {
        let any_radio_on = self.mobile_modems.get().is_some_and(|mobile_modems| {
            mobile_modems
                .modems
                .iter()
                .any(|modem| modem.radio_power() == RadioPower::On)
        });
        if any_radio_on {
            RadioPower::On
        } else {
            RadioPower::Off
        }
    }

    fn all_radios_hover_card(&self, radio_power: RadioPower) -> Vec<Section> {
        self.mobile_modems
            .hover_card_with(AllMobileRadiosSwitch { radio_power }.hover_card())
    }
}
