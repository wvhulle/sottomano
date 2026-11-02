use sottomano_sources::systemd_units_journal::graphical_session_target::GraphicalSessionTarget;

use crate::{
    clock::Clock,
    widgets::{
        group_item_reading::GroupItemReading,
        hover_card_body::{HoverCardBody, ReadingSections, Section},
        icon_glyph::IconGlyph,
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
};

pub struct DesktopServicesFace;

impl GroupItemReading for DesktopServicesFace {
    fn name(&self) -> &'static str {
        "Desktop services"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::ListChecks(Weight::Regular).into()
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([Section::dictionary([LabeledValue::new(
            "Started by",
            "systemd --user",
        )])
        .titled((
            "Services that need the desktop",
            "The panel, notifications and other services that need a running compositor.",
        ))])
    }
}

impl ReadingSections for GraphicalSessionTarget {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        let entries = [
            Some(LabeledValue::new("State", self.state.clone())),
            self.reached_at.map(|at| {
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
        vec![Section::dictionary(entries)]
    }
}
