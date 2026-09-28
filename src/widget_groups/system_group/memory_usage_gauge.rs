use crate::{
    data::source::sysfs_sensors::memory::Memory,
    widgets::{
        bar_widget::{BarWidget, Glyph, Level, Unit, Value},
        font_awesome::Solid,
        tooltip_body::{TooltipBody, row},
    },
};

impl BarWidget for Memory {
    fn icon(&self) -> Glyph {
        Solid::Memory.into()
    }

    fn level(&self) -> Level {
        match self.percent() {
            p if p > 90 => Level::Critical,
            p if p > 75 => Level::Warning,
            _ => Level::Ok,
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
