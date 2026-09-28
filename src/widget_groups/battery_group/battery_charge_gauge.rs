use crate::{
    data::source::sysfs_sensors::battery::Battery,
    widgets::{
        bar_widget::{BarWidget, Glyph, Level, Unit, Value},
        font_awesome::Solid,
        tooltip_body::{TooltipBody, row},
    },
};

impl BarWidget for Battery {
    fn icon(&self) -> Glyph {
        Solid::CarBattery.into()
    }

    fn level(&self) -> Level {
        match self.capacity {
            0..=15 => Level::Critical,
            16..=30 => Level::Warning,
            _ => Level::Ok,
        }
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Gauge {
            percent: i64::from(self.capacity),
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        let mut rows = vec![
            row("Battery", format!("{}%", self.capacity)),
            row("Status", self.status.as_str()),
        ];
        rows.extend(
            self.energy_wh()
                .map(|wh| row("Remaining", format!("{wh:.1} Wh"))),
        );
        TooltipBody::rows(rows)
    }
}
