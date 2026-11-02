use sottomano_config::severity::Severity;
use sottomano_sources::network_connectivity::{
    network_manager_proxies::Connectivity, nm_uplink_connectivity::UplinkConnectivity,
};

use crate::{
    clock::Clock,
    widgets::{
        group_item_reading::GroupItemReading,
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        reading_display_state::ReadingDisplayState,
        tooltip_body::TooltipBody,
        value_cells::LabeledValue,
    },
};

pub(super) const CONNECTIVITY_ITEM_NAME: &str = "Connectivity";

fn severity(connectivity: Connectivity) -> Severity {
    match connectivity {
        Connectivity::Full => Severity::Normal,
        Connectivity::Portal | Connectivity::Limited => Severity::Warning,
        Connectivity::Offline => Severity::Critical,
    }
}

impl GroupItemReading for UplinkConnectivity {
    fn name(&self) -> &'static str {
        CONNECTIVITY_ITEM_NAME
    }

    fn icon(&self) -> IconGlyph {
        match self.connectivity {
            Connectivity::Full => Phosphor::CloudCheck(Weight::Regular),
            Connectivity::Portal | Connectivity::Limited => Phosphor::Cloud(Weight::Regular),
            Connectivity::Offline => Phosphor::CloudSlash(Weight::Regular),
        }
        .into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        severity(self.connectivity).into()
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::dictionary([LabeledValue::new("Internet", self.connectivity.label())
            .level(severity(self.connectivity))])
    }
}
