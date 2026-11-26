use sottomano_desktop_sources::network_connectivity::modem_manager_mobile::{
    BearerTraffic, Mobile,
};

use super::MobileModemsGroup;
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{group_indicator::GroupIndicator, indicator_state::IndicatorState},
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{localized_quantities::Quantity, value_cells::LabeledValue},
};
pub struct ModemDataIndicator<'view> {
    pub modem: &'view Mobile,
}

impl<'view> ModemDataIndicator<'view> {
    pub fn indicator(self) -> GroupIndicator<'view> {
        GroupIndicator::glyph_only(
            "Mobile data",
            Phosphor::ArrowsDownUp(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        let BearerTraffic::Counted { received, sent } = self.modem.traffic else {
            return vec![Section::new("Transferred", dictionary_table(Vec::new()))];
        };
        vec![Section::new(
            "Transferred",
            dictionary_table([
                LabeledValue::new("Received (RX)", Quantity::size(received)),
                LabeledValue::new("Sent (TX)", Quantity::size(sent)),
            ]),
        )]
    }
}

impl MobileModemsGroup {
    pub(super) fn modem_data_rows<'view>(
        &'view self,
        modem: &'view Mobile,
    ) -> Vec<GroupIndicator<'view>> {
        let BearerTraffic::Counted { .. } = modem.traffic else {
            return Vec::new();
        };
        vec![
            ModemDataIndicator { modem }
                .indicator()
                .with_hover_card(Box::new(move |_| {
                    self.mobile_modems
                        .hover_card_with(ModemDataIndicator { modem }.hover_card())
                })),
        ]
    }
}
