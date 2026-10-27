use bytesize::ByteSize;
use sottomano_config::severity::Severity;
use sottomano_sources::disk_storage::root_filesystem_usage::{Filesystem, RootFilesystem};
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph, OngoingToastThreshold},
        gauge::percent_fill,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

const FREE_WARN: ByteSize = ByteSize::gb(100);
const FREE_CRITICAL: ByteSize = ByteSize::gb(2);
const UNALLOCATED_WARN: ByteSize = ByteSize::gib(1);

impl BarWidget for RootFilesystem {
    fn name(&self) -> &'static str {
        "Disk usage"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Database(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> WidgetState {
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

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        let capacity = Section::dictionary([
            KeyValue::new("Mount", "/".to_owned()),
            KeyValue::new("Used", Quantity::share(percent_fill(self.used_pct()))),
            KeyValue::new("Free", Quantity::size(self.free)),
            KeyValue::new("Total", Quantity::size(self.total)),
        ]);
        TooltipBody::sections([capacity, Section::from(&self.filesystem)])
    }
}

impl From<&Filesystem> for Section {
    fn from(filesystem: &Filesystem) -> Self {
        match filesystem {
            Filesystem::Btrfs(space) => Section::dictionary([
                KeyValue::new("Compression", space.compression.to_string()),
                KeyValue::new("Unallocated space", Quantity::size(space.unallocated)),
                KeyValue::new("Metadata in use", Quantity::size(space.metadata.used)),
                KeyValue::new(
                    "Metadata reserved",
                    Quantity::size(space.metadata.allocated),
                ),
            ])
            .titled("Btrfs"),
            Filesystem::Other { fs_type } => {
                Section::dictionary([KeyValue::new("Filesystem", fs_type.clone())])
            }
        }
    }
}
