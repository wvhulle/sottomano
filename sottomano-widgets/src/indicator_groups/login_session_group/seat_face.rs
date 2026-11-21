use sottomano_desktop_sources::login_session_presence::logind_session_facts::VirtualTerminal;

use crate::{
    glyph_icons::{
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
    },
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table},
    widgets::value_cells::{LabeledValue, ValueCell},
};

pub struct SeatFace<'view> {
    pub seat: &'view str,
    pub virtual_terminal: VirtualTerminal,
}

impl<'view> SeatFace<'view> {
    pub fn indicator(self) -> GroupIndicator<'view> {
        let label_text = match self.virtual_terminal {
            VirtualTerminal::Console(console) => format!("tty{console}"),
            VirtualTerminal::NoConsole => self.seat.to_owned(),
        };
        let label = if label_text.is_empty() {
            ReadoutLabel::Unlabeled
        } else {
            ReadoutLabel::Labeled(label_text)
        };
        GroupIndicator::glyph_only(
            "Seat",
            Phosphor::Keyboard(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_readout(IndicatorReadout {
            label,
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
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
        let VirtualTerminal::Console(console) = self.virtual_terminal else {
            return vec![seat];
        };
        vec![
            seat,
            Section::new(
                SectionTitle::captioned(
                    "Virtual console this login started on",
                    "The text terminal this login started on; the compositor took it over.",
                ),
                dictionary_table([
                    LabeledValue::new("Virtual console", format!("tty{console}")),
                    LabeledValue::new("Switch with", format!("Ctrl+Alt+F{console}")),
                    LabeledValue::new(
                        "Wiki",
                        ValueCell::BrandedLink {
                            label: "Linux console".to_owned(),
                            url: "https://wiki.archlinux.org/title/Linux_console".to_owned(),
                            glyph: SimpleIcon::ArchLinux.into(),
                        },
                    ),
                ]),
            ),
        ]
    }
}
