use crate::{
    data::source::sysfs_sensors::cpu_usage::CpuUsage,
    widgets::{
        bar_widget::{BarWidget, Glyph, Unit, Value},
        phosphor::Phosphor,
        tooltip_body::{TooltipBody, row},
        widget_state::WidgetState,
    },
};

impl BarWidget for CpuUsage {
    fn icon(&self) -> Glyph {
        Phosphor::Cpu(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        match self.percent {
            p if p > 90 => WidgetState::Critical,
            p if p > 70 => WidgetState::Warning,
            _ => WidgetState::Idle,
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
