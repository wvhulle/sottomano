use bytesize::ByteSize;
use uom::si::f64::Ratio;

use crate::{
    data::{severity::Severity, source::disk::Disk},
    widgets::{
        bar_widget::{BarWidget, Glyph, ReadingAlertThreshold},
        gauge::percent_fill,
        phosphor::Phosphor,
        tooltip_body::{TooltipBody, row},
        widget_state::WidgetState,
    },
};

const FREE_WARN: ByteSize = ByteSize::gb(100);
const FREE_CRITICAL: ByteSize = ByteSize::gb(2);

impl BarWidget for Disk {
    fn icon(&self) -> Glyph {
        Phosphor::HardDrive(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        if self.free < FREE_CRITICAL {
            WidgetState::Critical
        } else if self.free < FREE_WARN {
            WidgetState::Warning
        } else {
            WidgetState::Idle
        }
    }

    fn reading_alert_threshold(&self) -> ReadingAlertThreshold {
        ReadingAlertThreshold::NotifyFromSeverity {
            minimum_severity: Severity::Critical,
            summary_text: "Disk almost full",
        }
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(percent_fill(self.used_pct()))
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::rows(vec![
            row("Mount", "/"),
            row("Used", format!("{}%", self.used_pct())),
            row("Free", self.free.display().iec().to_string()),
            row("Total", self.total.display().iec().to_string()),
        ])
    }
}
