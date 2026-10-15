use crate::widgets::{
    bar_widget::{BarWidget, Glyph},
    phosphor::{Phosphor, Weight},
    tooltip_body::{KeyValue, Section, TooltipBody},
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
