use uom::si::f64::Ratio;

use crate::{
    data::source::sysfs_sensors::memory::Memory,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        gauge::percent_fill,
        phosphor::Phosphor,
        tooltip_body::{TooltipBody, row},
        widget_state::WidgetState,
    },
};

impl BarWidget for Memory {
    fn icon(&self) -> Glyph {
        Phosphor::Memory(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        match self.percent() {
            p if p > 90 => WidgetState::Critical,
            p if p > 75 => WidgetState::Warning,
            _ => WidgetState::Idle,
        }
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(percent_fill(self.percent()))
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::rows(vec![
            row("Memory", format!("{}%", self.percent())),
            row("Used", self.used.display().iec().to_string()),
            row("Total", self.total.display().iec().to_string()),
        ])
    }
}
