use std::num::NonZeroU32;

use chrono::Locale;

use crate::{
    clock::Clock,
    group_items::group_item_reading::GroupItemReading,
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        value_cells::{LabeledValue, ValueCell},
    },
};

pub struct SeatFace<'a> {
    pub seat: &'a str,
    pub virtual_terminal: Option<NonZeroU32>,
}

impl GroupItemReading for SeatFace<'_> {
    fn name(&self) -> &'static str {
        "Seat"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Keyboard(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(
            self.virtual_terminal
                .map_or_else(|| self.seat.to_owned(), |vt| format!("tty{vt}")),
        )
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let seat = Section::dictionary([
            LabeledValue::new("Hardware seat", self.seat.to_owned()),
            LabeledValue::new(
                "Wiki",
                ValueCell::BrandedLink {
                    label: "Multiseat".to_owned(),
                    url: "https://wiki.archlinux.org/title/Multiseat".to_owned(),
                    glyph: SimpleIcon::ArchLinux.into(),
                },
            ),
        ])
        .titled((
            "Hardware seat in use",
            "The screen, keyboard and mouse this login owns.",
        ));
        let console = self.virtual_terminal.map(|vt| {
            Section::dictionary([
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
            ])
            .titled((
                "Virtual console this login started on",
                "The text terminal this login started on; the compositor took it over.",
            ))
        });
        HoverCardBody::sections([seat].into_iter().chain(console))
    }
}
