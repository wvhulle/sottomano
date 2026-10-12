use uom::si::f64::Ratio;

use crate::{
    data::{severity::Severity, source::sysfs_sensors::memory::Memory},
    widgets::{
        bar_widget::{BarWidget, Glyph, OngoingToastThreshold},
        gauge::percent_fill,
        phosphor::Phosphor,
        tooltip_body::{KeyValue, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for Memory {
    fn name(&self) -> &'static str {
        "Memory"
    }

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

    fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
        OngoingToastThreshold::NotifyFromSeverity {
            minimum_severity: Severity::Critical,
            summary_text: "Memory almost full",
        }
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(percent_fill(self.percent()))
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::dictionary([
            KeyValue::new("Memory", format!("{}%", self.percent())),
            KeyValue::new("Used", self.used.display().iec().to_string()),
            KeyValue::new("Total", self.total.display().iec().to_string()),
        ])
    }
}
