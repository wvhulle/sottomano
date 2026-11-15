use sottomano_desktop_sources::login_session_presence::logind_session_facts::SessionIdentity;

use crate::{
    clock::Clock,
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table},
    widgets::{
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

pub struct AccountFace<'a> {
    pub identity: &'a SessionIdentity,
}

impl<'a> AccountFace<'a> {
    pub fn reading_row(self) -> ReadingRow<'a> {
        let label_text = self.identity.user.clone();
        let label = if label_text.is_empty() {
            ReadoutLabel::Unlabeled
        } else {
            ReadoutLabel::Labeled(label_text)
        };
        ReadingRow {
            readout: RowReadout {
                label,
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                "Account",
                Phosphor::User(Weight::Regular).into(),
                ReadingDisplayState::Idle,
                Box::new(move |clock| self.hover_card(clock)),
            )
        }
    }

    pub fn hover_card(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::new(
            SectionTitle::captioned(
                "Account this session runs as",
                "The user this session runs as; everything above runs with its permissions.",
            ),
            dictionary_table([
                LabeledValue::new("User", self.identity.user.clone()),
                LabeledValue::new("User id", self.identity.uid.to_string()),
            ]),
        )]
    }
}
