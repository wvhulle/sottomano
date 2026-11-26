use sottomano_toml_config::Config;

use super::{ColorTemperatureBusRequest, ColorTemperatureGroup, ColorTemperatureInteraction};
use crate::{
    group_indicators::group_indicator::GroupIndicator,
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::value_cells::LabeledValue,
};

impl ColorTemperatureGroup {
    pub(super) fn warmth_rows<'view>(
        &'view self,
        config: &'view Config,
    ) -> Vec<GroupIndicator<'view>> {
        if self.gamma.get().is_none() {
            return Vec::new();
        }
        vec![
            GroupIndicator::slider(&self.warmth, config, ColorTemperatureInteraction::Warmth)
                .with_hover_card(Box::new(move |_| {
                    self.gamma.hover_card_with(vec![Section::new(
                        "Setting",
                        dictionary_table([LabeledValue::new(
                            "Temperature",
                            self.warmth.readout_cell(),
                        )]),
                    )])
                }))
                .refreshed_by(ColorTemperatureInteraction::OnBuses(
                    ColorTemperatureBusRequest::Refresh,
                )),
        ]
    }
}
