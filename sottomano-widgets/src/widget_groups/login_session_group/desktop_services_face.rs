use sottomano_sources::systemd_units_journal::graphical_session_target::GraphicalSessionTarget;

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        tooltip_body::{ReadingSections, Section, TooltipBody},
        value_cells::{LabeledValue, ValueCell},
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
        TooltipBody::sections([Section::dictionary([LabeledValue::new(
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
