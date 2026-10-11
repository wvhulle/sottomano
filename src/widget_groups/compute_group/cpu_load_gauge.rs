use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Ratio, ratio::percent},
};

use super::core_kind_sections::{PerCore, sections_by_core_kind};
use crate::{
    data::{
        severity::Severity,
        source::sysfs_sensors::cpu_usage::{CpuUsage, LoadSplit},
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::Phosphor,
        tooltip_body::{Cell, KeyValue, Section, TableRecord, TooltipBody},
        widget_state::WidgetState,
    },
};

fn busy_level(busy: Ratio) -> Severity {
    if busy > Ratio::new::<percent>(90.0) {
        Severity::Critical
    } else if busy > Ratio::new::<percent>(70.0) {
        Severity::Warning
    } else {
        Severity::Normal
    }
}

fn percent_text(fraction: Ratio) -> String {
    format!("{:.0}", fraction.into_format_args(percent, Abbreviation))
}

struct CoreReading {
    core: u32,
    busy: Ratio,
    split: LoadSplit,
    deep_idle: Option<Ratio>,
}

impl CpuUsage {
    fn core_readings(&self) -> Vec<CoreReading> {
        self.cores
            .iter()
            .map(|load| CoreReading {
                core: load.core,
                busy: load.busy,
                split: load.split,
                deep_idle: self
                    .deep_idle
                    .iter()
                    .find(|idle| idle.core == load.core)
                    .map(|idle| idle.share),
            })
            .collect()
    }
}

impl PerCore for CoreReading {
    fn core(&self) -> u32 {
        self.core
    }
}

impl CoreReading {
    fn table_record(&self) -> TableRecord<6> {
        let share = |share: Ratio| Some(Cell::from(percent_text(share)));
        TableRecord::new(
            format!("Core {}", self.core),
            [
                share(self.busy),
                share(self.split.user),
                share(self.split.system),
                share(self.split.iowait),
                share(self.split.irq),
                self.deep_idle.and_then(share),
            ],
        )
        .level(busy_level(self.busy))
    }
}

impl BarWidget for CpuUsage {
    fn name(&self) -> &'static str {
        "CPU load"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Cpu(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        busy_level(self.busy).into()
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(self.busy)
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(
            &[
                vec![Section::untitled_dictionary([KeyValue::new(
                    "CPU",
                    percent_text(self.busy),
                )])],
                sections_by_core_kind(
                    &self.topology,
                    self.core_readings(),
                    ["Load", "User", "Sys", "IO", "IRQ", "Idle"],
                    CoreReading::table_record,
                ),
            ]
            .concat(),
        )
    }
}
