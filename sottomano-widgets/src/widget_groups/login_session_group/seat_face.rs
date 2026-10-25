use std::num::NonZeroU32;

use chrono::Locale;

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        tooltip_body::{Cell, KeyValue, Section, TooltipBody},
    },
};

pub struct SeatFace<'a> {
    pub seat: &'a str,
    pub virtual_terminal: Option<NonZeroU32>,
}

impl BarWidget for SeatFace<'_> {
    fn name(&self) -> &'static str {
        "Seat"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Keyboard(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(
            self.virtual_terminal
                .map_or_else(|| self.seat.to_owned(), |vt| format!("tty{vt}")),
        )
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        let seat = Section::dictionary([
            KeyValue::new("Hardware seat", self.seat.to_owned()),
            KeyValue::new(
                "Wiki",
                Cell::Link {
                    label: "Multiseat".to_owned(),
                    url: "https://wiki.archlinux.org/title/Multiseat".to_owned(),
                    glyph: Some(SimpleIcon::ArchLinux.into()),
                },
            ),
        ])
        .titled((
            "Hardware seat in use",
            "The screen, keyboard and mouse this login owns.",
        ));
        let console = self.virtual_terminal.map(|vt| {
            Section::dictionary([
                KeyValue::new("Virtual console", format!("tty{vt}")),
                KeyValue::new("Switch with", format!("Ctrl+Alt+F{vt}")),
                KeyValue::new(
                    "Wiki",
                    Cell::Link {
                        label: "Linux console".to_owned(),
                        url: "https://wiki.archlinux.org/title/Linux_console".to_owned(),
                        glyph: Some(SimpleIcon::ArchLinux.into()),
                    },
                ),
            ])
            .titled((
                "Virtual console this login started on",
                "The text terminal this login started on; the compositor took it over.",
            ))
        });
        TooltipBody::sections([seat].into_iter().chain(console))
    }
}
