use std::borrow::Cow;

use sottomano_desktop_sources::{
    SnapshotSource,
    voice_dictation_input::notulist_dbus_interface_mismatch::NotulistInterfaceMismatch,
};

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading, reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

const MISMATCH_REMEDY: &str =
    "build sottomano and notulist against the same notulist-client revision";

pub(crate) const NOTULIST_INTERFACE_MISMATCH_FALLBACK_GLYPH: IconGlyph =
    IconGlyph::Phosphor(Phosphor::LinkBreak(Weight::Regular));

impl GroupItemReading for NotulistInterfaceMismatch {
    fn name(&self) -> Cow<'static, str> {
        Self::NAME.into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::LinkBreak(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::Failed
    }

    fn hover_card(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::new(
            "Versions",
            dictionary_table(interface_mismatch_rows(self)),
        )]
    }
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
