use sottomano_desktop_sources::systemd_units_journal::graphical_session_target::GraphicalSessionTarget;

use crate::{
    group_items::{reading_display_state::ReadingDisplayState, reading_row::ReadingRow},
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table},
    widgets::{
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
};

pub struct DesktopServicesFace;

impl DesktopServicesFace {
    pub fn reading_row() -> ReadingRow<'static> {
        ReadingRow::glyph_only(
            "Desktop services",
            Phosphor::ListChecks(Weight::Regular).into(),
            ReadingDisplayState::Idle,
            Box::new(|_| Self::hover_card()),
        )
    }

    pub fn hover_card() -> Vec<Section> {
        vec![Section::new(
            SectionTitle::captioned(
                "Services that need the desktop",
                "The panel, notifications and other services that need a running compositor.",
            ),
            dictionary_table([LabeledValue::new("Started by", "systemd --user")]),
        )]
    }
}

pub fn graphical_session_target_sections(
    graphical_session_target: &GraphicalSessionTarget,
) -> Vec<Section> {
    let entries = [
        Some(LabeledValue::new(
            "State",
            graphical_session_target.state.clone(),
        )),
        graphical_session_target.reached_at.map(|at| {
            LabeledValue::new(
                "Ready since",
                ValueCell::Time {
                    at,
                    format: LocalizedTimeFormat::DateAndTime,
                },
            )
        }),
    ]
    .into_iter()
    .flatten();
    vec![Section::new(
        SectionTitle::captioned(
            "Target state",
            "graphical-session.target is reached once the compositor is ready for services.",
        ),
        dictionary_table(entries),
    )]
}
