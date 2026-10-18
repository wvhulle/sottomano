use bytesize::ByteSize;
use uom::si::f64::Ratio;
use vetrata_config::severity::Severity;
use vetrata_sources::disk::{Disk, Filesystem};

use crate::widgets::{
    bar_widget::{BarWidget, Glyph, OngoingToastThreshold},
    gauge::percent_fill,
    phosphor::Phosphor,
    tooltip_body::{KeyValue, Section, TooltipBody},
    widget_state::WidgetState,
};

const FREE_WARN: ByteSize = ByteSize::gb(100);
const FREE_CRITICAL: ByteSize = ByteSize::gb(2);
const UNALLOCATED_WARN: ByteSize = ByteSize::gib(1);

impl BarWidget for Disk {
    fn name(&self) -> &'static str {
        "Disk usage"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Database(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        let free_space_level = if self.free < FREE_CRITICAL {
            Severity::Critical
        } else if self.free < FREE_WARN {
            Severity::Warning
        } else {
            Severity::Normal
        };
        let unallocated_level = match &self.filesystem {
            Filesystem::Btrfs(space) if space.unallocated < UNALLOCATED_WARN => Severity::Warning,
            Filesystem::Btrfs(_) | Filesystem::Other { .. } => Severity::Normal,
        };
        free_space_level.max(unallocated_level).into()
    }

    fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
        OngoingToastThreshold::NotifyFromSeverity {
            minimum_severity: Severity::Critical,
            summary_text: "Disk almost full",
        }
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(percent_fill(self.used_pct()))
    }

    fn tooltip(&self) -> TooltipBody {
        let capacity = Section::dictionary([
            KeyValue::new("Mount", "/".to_owned()),
            KeyValue::new("Used", format!("{}%", self.used_pct())),
            KeyValue::new("Free", self.free.display().iec().to_string()),
            KeyValue::new("Total", self.total.display().iec().to_string()),
        ]);
        TooltipBody::sections([capacity, Section::from(&self.filesystem)])
    }
}

impl From<&Filesystem> for Section {
    fn from(filesystem: &Filesystem) -> Self {
        match filesystem {
            Filesystem::Btrfs(space) => Section::dictionary([
                KeyValue::new("Compression", space.compression.to_string()),
                KeyValue::new(
                    "Unallocated space",
                    space.unallocated.display().iec().to_string(),
                ),
                KeyValue::new(
                    "Metadata in use",
                    space.metadata.used.display().iec().to_string(),
                ),
                KeyValue::new(
                    "Metadata reserved",
                    space.metadata.allocated.display().iec().to_string(),
                ),
            ])
            .titled("Btrfs"),
            Filesystem::Other { fs_type } => {
                Section::dictionary([KeyValue::new("Filesystem", fs_type.clone())])
            }
        }
    }
}
