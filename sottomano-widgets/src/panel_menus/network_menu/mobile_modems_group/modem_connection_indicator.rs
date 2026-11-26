use sottomano_desktop_sources::network_connectivity::{
    modem_manager_mobile::{Mobile, ModemFailure, ModemState},
    network_manager_proxies::{LinkPhase, RadioPower},
};

use super::{MobileModemsGroup, MobileModemsInteraction};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{group_indicator::GroupIndicator, indicator_state::IndicatorState},
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::value_cells::LabeledValue,
};
pub struct ModemConnectionSwitch<'view> {
    pub modem: &'view Mobile,
}

impl<'view> ModemConnectionSwitch<'view> {
    #[must_use]
    pub fn press_does(&self) -> &'static str {
        if self.modem.link_state() == LinkPhase::Up {
            "disconnect this modem"
        } else {
            "connect this modem"
        }
    }

    pub fn indicator(self) -> GroupIndicator<'view> {
        let glyph = if self.modem.link_state() == LinkPhase::Up {
            Phosphor::ToggleRight(Weight::Regular).into()
        } else {
            Phosphor::ToggleLeft(Weight::Regular).into()
        };
        GroupIndicator::glyph_only(
            "Connection",
            glyph,
            IndicatorState::from(self.modem.link_state()),
        )
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        let mut status_entries = Vec::new();
        if self.modem.state != ModemState::Unreported {
            status_entries.push(LabeledValue::new(
                "Connection State",
                self.modem.state.label(),
            ));
        }
        if let ModemState::Failed { reason } = self.modem.state
            && reason != ModemFailure::Unspecified
        {
            status_entries.push(LabeledValue::new("Failure Reason", reason.label()));
        }
        vec![Section::new("Status", dictionary_table(status_entries))]
    }
}

impl MobileModemsGroup {
    pub(super) fn modem_connection_rows<'view>(
        &'view self,
        modem: &'view Mobile,
    ) -> Vec<GroupIndicator<'view>> {
        if modem.radio_power() != RadioPower::On {
            return Vec::new();
        }
        let connection_switch = ModemConnectionSwitch { modem };
        let connection_press_does = connection_switch.press_does();
        vec![
            connection_switch
                .indicator()
                .with_hover_card(Box::new(move |_| self.modem_connection_hover_card(modem)))
                .on_switch_press(
                    modem.link_state(),
                    connection_press_does,
                    MobileModemsInteraction::ToggleModem(modem.modem_path.clone()),
                ),
        ]
    }

    fn modem_connection_hover_card(&self, modem: &Mobile) -> Vec<Section> {
        self.mobile_modems
            .hover_card_with(ModemConnectionSwitch { modem }.hover_card())
    }
}
