use crate::{
    data::source::sysfs_sensors::{cpu_usage::CpuUsage, favg, fmax, fmin},
    widget_groups::system_group::cpu_clock_label::ghz,
    widgets::{
        bar_widget::{BarWidget, Glyph, Level, Unit, Value},
        font_awesome::Solid,
        tooltip_body::{TooltipBody, row},
    },
};

impl BarWidget for CpuUsage {
    fn icon(&self) -> Glyph {
        Solid::Microchip.into()
    }

    fn level(&self) -> Level {
        match self.percent {
            p if p > 90 => Level::Critical,
            p if p > 70 => Level::Warning,
            _ => Level::Ok,
        }
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Gauge {
            percent: self.percent,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        let mut rows = vec![row("CPU", format!("{}%", self.percent))];
        if self.freqs_ghz.is_empty() {
            rows.push(row("Frequency", "unavailable"));
        } else {
            rows.push(row("Frequency min", ghz(fmin(&self.freqs_ghz))));
            rows.push(row("Frequency avg", ghz(favg(&self.freqs_ghz))));
            rows.push(row("Frequency max", ghz(fmax(&self.freqs_ghz))));
        }
        TooltipBody::rows(rows)
    }
}
