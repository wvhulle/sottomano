use crate::{
    data::source::sysfs_sensors::cpu_usage::CpuUsage,
    widgets::{
        bar_widget::{BarWidget, Glyph, Level, Unit, Value},
        phosphor,
        tooltip_body::{TooltipBody, row},
    },
};

impl BarWidget for CpuUsage {
    fn icon(&self) -> Glyph {
        phosphor::Cpu::Regular.into()
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
        let frequencies = self.clock.summary_rows("Frequency");
        let frequencies = if frequencies.is_empty() {
            vec![row("Frequency", "unavailable")]
        } else {
            frequencies
        };
        TooltipBody::rows(
            [row("CPU", format!("{}%", self.percent))]
                .into_iter()
                .chain(frequencies)
                .collect(),
        )
    }
}
