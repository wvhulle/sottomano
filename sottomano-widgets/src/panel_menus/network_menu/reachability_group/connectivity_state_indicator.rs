use sottomano_desktop_sources::{
    SnapshotSource,
    network_connectivity::{
        network_manager_proxies::Connectivity, nm_uplink_connectivity::UplinkConnectivity,
    },
};
use sottomano_toml_config::severity::Severity;

use super::ReachabilityGroup;
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator, indicator_group_content::IndicatorListing,
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table},
    widgets::value_cells::LabeledValue,
};

pub(super) const CONNECTIVITY_GLYPH: Phosphor = Phosphor::CloudCheck(Weight::Regular);

fn severity(connectivity: Connectivity) -> Severity {
    match connectivity {
        Connectivity::Full => Severity::Normal,
        Connectivity::Portal | Connectivity::Limited => Severity::Warning,
        Connectivity::Offline => Severity::Critical,
    }
}

fn uplink_connectivity_hover_card(uplink: UplinkConnectivity) -> Vec<Section> {
    vec![Section::new(
        SectionTitle::captioned(
            "Status",
            "NetworkManager's own check whether the internet answers or a login portal is in the \
             way.",
        ),
        dictionary_table([LabeledValue::new("Internet", uplink.connectivity.label())
            .level(severity(uplink.connectivity))]),
    )]
}

pub(super) fn uplink_connectivity_indicator(uplink: &UplinkConnectivity) -> GroupIndicator<'_> {
    let glyph = match uplink.connectivity {
        Connectivity::Full => Phosphor::CloudCheck(Weight::Regular),
        Connectivity::Portal | Connectivity::Limited => Phosphor::Cloud(Weight::Regular),
        Connectivity::Offline => Phosphor::CloudSlash(Weight::Regular),
    };
    GroupIndicator::glyph_only(
        UplinkConnectivity::NAME,
        glyph.into(),
        severity(uplink.connectivity).into(),
    )
    .with_hover_card(Box::new(move |_| uplink_connectivity_hover_card(*uplink)))
}

impl ReachabilityGroup {
    pub(super) fn connectivity_listing(&self) -> IndicatorListing<'_> {
        self.connectivity.indicator_listing(
            |held_snapshot| vec![uplink_connectivity_indicator(held_snapshot)],
            CONNECTIVITY_GLYPH.into(),
        )
    }
}
