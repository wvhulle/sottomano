use sottomano_desktop_sources::{
    SnapshotSource,
    voice_dictation_input::notulist_dbus_interface_mismatch::NotulistInterfaceMismatch,
};

use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::{group_indicator::GroupIndicator, indicator_state::IndicatorState},
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::value_cells::LabeledValue,
};

const MISMATCH_REMEDY: &str =
    "build sottomano and notulist against the same notulist-client revision";

pub(crate) const NOTULIST_INTERFACE_MISMATCH_FALLBACK_GLYPH: IconGlyph =
    IconGlyph::Phosphor(Phosphor::LinkBreak(Weight::Regular));

pub fn notulist_interface_mismatch_indicator(
    mismatch: &NotulistInterfaceMismatch,
) -> GroupIndicator<'_> {
    GroupIndicator::glyph_only(
        NotulistInterfaceMismatch::NAME,
        Phosphor::LinkBreak(Weight::Regular).into(),
        IndicatorState::Failed,
    )
    .with_hover_card(Box::new(move |_| {
        notulist_interface_mismatch_hover_card(mismatch)
    }))
}

pub fn notulist_interface_mismatch_hover_card(
    mismatch: &NotulistInterfaceMismatch,
) -> Vec<Section> {
    vec![Section::new(
        "Versions",
        dictionary_table(interface_mismatch_rows(mismatch)),
    )]
}

fn interface_mismatch_rows(mismatch: &NotulistInterfaceMismatch) -> [LabeledValue; 3] {
    let offered_description = if mismatch.offered_versions.is_empty() {
        "no version of this interface".to_owned()
    } else {
        mismatch.offered_versions.join(", ")
    };
    [
        LabeledValue::new("Expected", mismatch.expected_interface.clone()),
        LabeledValue::new("Offered", offered_description),
        LabeledValue::new("Fix", MISMATCH_REMEDY),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_rows_name_expected_and_offered_interfaces() {
        let mismatch = NotulistInterfaceMismatch {
            expected_interface: "page.example.Dictation4".to_owned(),
            offered_versions: vec!["page.example.Dictation5".to_owned()],
        };
        assert_eq!(
            interface_mismatch_rows(&mismatch),
            [
                LabeledValue::new("Expected", "page.example.Dictation4"),
                LabeledValue::new("Offered", "page.example.Dictation5"),
                LabeledValue::new("Fix", MISMATCH_REMEDY),
            ]
        );
    }

    #[test]
    fn missing_interface_family_says_no_version_is_offered() {
        let mismatch = NotulistInterfaceMismatch {
            expected_interface: "page.example.Dictation4".to_owned(),
            offered_versions: Vec::new(),
        };
        assert_eq!(
            interface_mismatch_rows(&mismatch)[1],
            LabeledValue::new("Offered", "no version of this interface")
        );
    }
}
