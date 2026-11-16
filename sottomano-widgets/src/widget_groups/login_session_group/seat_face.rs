use std::num::NonZeroU32;

use crate::{
    group_indicators::{
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
        reading_row::ReadingRow,
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table},
    widgets::{
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        value_cells::{LabeledValue, ValueCell},
    },
};

pub struct SeatFace<'a> {
    pub seat: &'a str,
    pub virtual_terminal: Option<NonZeroU32>,
}

impl<'a> SeatFace<'a> {
    pub fn reading_row(self) -> ReadingRow<'a> {
        let label_text = self
            .virtual_terminal
            .map_or_else(|| self.seat.to_owned(), |vt| format!("tty{vt}"));
        let label = if label_text.is_empty() {
            ReadoutLabel::Unlabeled
        } else {
            ReadoutLabel::Labeled(label_text)
        };
        ReadingRow {
            readout: IndicatorReadout {
                label,
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                "Seat",
                Phosphor::Keyboard(Weight::Regular).into(),
                IndicatorState::Idle,
                Box::new(move |_| self.hover_card()),
            )
        }
    }

    pub fn hover_card(&self) -> Vec<Section> {
        let seat = Section::new(
            SectionTitle::captioned(
                "Hardware seat in use",
                "The screen, keyboard and mouse this login owns.",
            ),
            dictionary_table([
                LabeledValue::new("Hardware seat", self.seat.to_owned()),
                LabeledValue::new(
                    "Wiki",
                    ValueCell::BrandedLink {
                        label: "Multiseat".to_owned(),
                        url: "https://wiki.archlinux.org/title/Multiseat".to_owned(),
                        glyph: SimpleIcon::ArchLinux.into(),
                    },
                ),
            ]),
        );
        let console = self.virtual_terminal.map(|vt| {
            Section::new(
                SectionTitle::captioned(
                    "Virtual console this login started on",
                    "The text terminal this login started on; the compositor took it over.",
                ),
                dictionary_table([
                    LabeledValue::new("Virtual console", format!("tty{vt}")),
                    LabeledValue::new("Switch with", format!("Ctrl+Alt+F{vt}")),
                    LabeledValue::new(
                        "Wiki",
                        ValueCell::BrandedLink {
                            label: "Linux console".to_owned(),
                            url: "https://wiki.archlinux.org/title/Linux_console".to_owned(),
                            glyph: SimpleIcon::ArchLinux.into(),
                        },
                    ),
                ]),
            )
        });
        [seat].into_iter().chain(console).collect::<Vec<_>>()
    }
}
