use chrono::Locale;
use sottomano_desktop_sources::{
    SnapshotSource,
    network_connectivity::{
        network_manager_proxies::Connectivity, nm_uplink_connectivity::UplinkConnectivity,
    },
};
use sottomano_toml_config::severity::Severity;

use crate::{
    clock::Clock,
    group_items::reading_row::ReadingRow,
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table},
    widgets::{
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

fn severity(connectivity: Connectivity) -> Severity {
    match connectivity {
        Connectivity::Full => Severity::Normal,
        Connectivity::Portal | Connectivity::Limited => Severity::Warning,
        Connectivity::Offline => Severity::Critical,
    }
}

fn uplink_connectivity_hover_card(uplink: UplinkConnectivity, _clock: Clock) -> Vec<Section> {
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

pub(super) fn uplink_connectivity_reading_row(
    uplink: &UplinkConnectivity,
    _clock: Clock,
    _locale: Locale,
) -> ReadingRow<'_> {
    let glyph = match uplink.connectivity {
        Connectivity::Full => Phosphor::CloudCheck(Weight::Regular),
        Connectivity::Portal | Connectivity::Limited => Phosphor::Cloud(Weight::Regular),
        Connectivity::Offline => Phosphor::CloudSlash(Weight::Regular),
    };
    ReadingRow::glyph_only(
        UplinkConnectivity::NAME,
        glyph.into(),
        severity(uplink.connectivity).into(),
        Box::new(move |card_clock| uplink_connectivity_hover_card(*uplink, card_clock)),
    )
}
