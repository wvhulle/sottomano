use bytesize::ByteSize;
use uom::si::f64::Ratio;

use crate::{
    data::{
        severity::Severity,
        source::disk::{Disk, Filesystem},
    },
    widgets::{
        bar_widget::{BarWidget, Glyph, ReadingAlertThreshold},
        gauge::percent_fill,
        phosphor::Phosphor,
        tooltip_body::{Section, TooltipBody, row, section, untitled},
        widget_state::WidgetState,
    },
};

const FREE_WARN: ByteSize = ByteSize::gb(100);
const FREE_CRITICAL: ByteSize = ByteSize::gb(2);
const UNALLOCATED_WARN: ByteSize = ByteSize::gib(1);

impl BarWidget for Disk {
    fn icon(&self) -> Glyph {
        Phosphor::HardDrive(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        self.free_space_level().max(self.unallocated_level()).into()
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
        let capacity = untitled(vec![
            row("Mount", "/"),
            row("Used", format!("{}%", self.used_pct())),
            row("Free", self.free.display().iec().to_string()),
            row("Total", self.total.display().iec().to_string()),
        ]);
        TooltipBody::Sections(vec![capacity, self.filesystem.section()])
    }
}

impl Disk {
    fn free_space_level(&self) -> Severity {
        if self.free < FREE_CRITICAL {
            Severity::Critical
        } else if self.free < FREE_WARN {
            Severity::Warning
        } else {
            Severity::Normal
        }
    }

    fn unallocated_level(&self) -> Severity {
        match &self.filesystem {
            Filesystem::Btrfs(space) if space.unallocated < UNALLOCATED_WARN => Severity::Warning,
            Filesystem::Btrfs(_) | Filesystem::Other { .. } => Severity::Normal,
        }
    }
}

impl Filesystem {
    fn section(&self) -> Section {
        match self {
            Filesystem::Btrfs(space) => section(
                "Btrfs",
                vec![
                    row("Compression", space.compression.to_string()),
                    row(
                        "Unallocated space",
                        space.unallocated.display().iec().to_string(),
                    ),
                    row(
                        "Metadata in use",
                        space.metadata.used.display().iec().to_string(),
                    ),
                    row(
                        "Metadata reserved",
                        space.metadata.allocated.display().iec().to_string(),
                    ),
                ],
            ),
            Filesystem::Other { fs_type } => untitled(vec![row("Filesystem", fs_type.clone())]),
        }
    }
}
