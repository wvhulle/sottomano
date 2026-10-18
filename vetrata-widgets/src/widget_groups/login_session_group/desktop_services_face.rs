use vetrata_sources::systemd_manager::graphical_session::GraphicalSessionTarget;

use crate::widgets::{
    bar_widget::{BarWidget, Glyph},
    localized_time::LocalizedTimeFormat,
    phosphor::{Phosphor, Weight},
    tooltip_body::{Cell, KeyValue, ReadingSections, Section, TooltipBody},
};

pub struct DesktopServicesFace;

impl BarWidget for DesktopServicesFace {
    fn name(&self) -> &'static str {
        "Desktop services"
    }

    fn icon(&self) -> Glyph {
        Phosphor::ListChecks(Weight::Regular).into()
    }

    fn tooltip(&self) -> TooltipBody {
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
    fn sections(&self) -> Vec<Section> {
        let entries = match self {
            Self::NotLoaded => vec![KeyValue::new("State", "not loaded")],
            Self::Loaded { state, reached_at } => [
                Some(KeyValue::new("State", state.clone())),
                reached_at.map(|at| {
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
            .flatten()
            .collect(),
        };
        vec![Section::dictionary(entries).titled((
            "Desktop startup progress",
            "graphical-session.target is reached once the compositor is ready for services.",
        ))]
    }
}
