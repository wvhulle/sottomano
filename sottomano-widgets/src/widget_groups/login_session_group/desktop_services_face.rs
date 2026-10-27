use sottomano_sources::systemd_units_journal::graphical_session_target::GraphicalSessionTarget;

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Cell, KeyValue, ReadingSections, Section, TooltipBody},
    },
};

pub struct DesktopServicesFace;

impl BarWidget for DesktopServicesFace {
    fn name(&self) -> &'static str {
        "Desktop services"
    }

    fn icon(&self) -> Glyph {
        Phosphor::ListChecks(Weight::Regular).into()
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([Section::dictionary([KeyValue::new(
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
            Some(KeyValue::new("State", self.state.clone())),
            self.reached_at.map(|at| {
                KeyValue::new(
                    "Ready since",
                    Cell::Time {
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
