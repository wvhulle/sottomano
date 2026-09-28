use crate::{
    data::source::sysfs_sensors::memory::Memory,
    widgets::{
        bar_widget::{BarWidget, Glyph, Unit, Value},
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

    fn value(&self) -> Option<Value> {
        Some(Value::Gauge {
            percent: self.percent(),
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::rows(vec![
            row("Memory", format!("{}%", self.percent())),
            row("Used", self.used.display().iec().to_string()),
            row("Total", self.total.display().iec().to_string()),
        ])
    }
}
