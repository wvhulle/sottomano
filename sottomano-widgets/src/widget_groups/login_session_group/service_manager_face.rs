use sottomano_desktop_sources::systemd_units_journal::user_service_manager::UserServiceManager;

use crate::{
    clock::Clock,
    group_items::{reading_display_state::ReadingDisplayState, reading_row::ReadingRow},
    hover_cards::hover_card_body::{ReadingSections, Section, SectionTitle, dictionary_table},
    widgets::{
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        value_cells::{LabeledValue, ValueCell},
    },
};

pub struct ServiceManagerFace;

impl ServiceManagerFace {
    pub fn reading_row() -> ReadingRow<'static> {
        ReadingRow::glyph_only(
            "Service manager",
            Phosphor::GearSix(Weight::Regular).into(),
            ReadingDisplayState::Idle,
            Box::new(Self::hover_card),
        )
    }

    pub fn hover_card(_clock: Clock) -> Vec<Section> {
        vec![Section::new(
            SectionTitle::captioned(
                "Per-user service manager",
                "Starts and supervises your services, the compositor included.",
            ),
            dictionary_table([
                LabeledValue::new("Service manager", "systemd --user"),
                LabeledValue::new(
                    "Wiki",
                    ValueCell::BrandedLink {
                        label: "systemd/User".to_owned(),
                        url: "https://wiki.archlinux.org/title/Systemd/User".to_owned(),
                        glyph: SimpleIcon::ArchLinux.into(),
                    },
                ),
            ]),
        )]
    }
}

impl ReadingSections for UserServiceManager {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::new(
            SectionTitle::captioned(
                "Status",
                "One instance per user; it outlives this login and stops at your last logout.",
            ),
            dictionary_table(
                [
                    Some(LabeledValue::new("systemd version", self.version.clone())),
                    Some(LabeledValue::new("State", self.state.clone())),
                    self.started_at.map(|at| {
                        LabeledValue::new(
                            "Running since",
                            ValueCell::Time {
                                at,
                                format: LocalizedTimeFormat::DateAndTime,
                            },
                        )
                    }),
                ]
                .into_iter()
                .flatten(),
            ),
        )]
    }
}
