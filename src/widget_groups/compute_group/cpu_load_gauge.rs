use uom::si::{f64::Ratio, ratio::percent};

use super::core_kind_sections::{PerCore, sections_by_core_kind};
use crate::{
    data::{
        severity::Severity,
        source::sysfs_sensors::{
            cpu_topology::CpuTopology,
            cpu_usage::{CpuCoreLoads, CpuUsage, LoadSplit},
        },
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        gauge::percent_text,
        phosphor::Phosphor,
        tooltip_body::{Cell, ColumnHeading, KeyValue, Section, TableRecord, TooltipBody},
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

struct CoreReading {
    core: u32,
    busy: Ratio,
    split: LoadSplit,
    deep_idle: Option<Ratio>,
}

impl CpuCoreLoads {
    pub fn sections(&self, topology: &CpuTopology) -> Vec<Section> {
        sections_by_core_kind(
            topology,
            self.core_readings(),
            [
                ColumnHeading::new("Load"),
                ColumnHeading::new("User"),
                ColumnHeading::new("Sys"),
                ColumnHeading::new("IO"),
                ColumnHeading::new("IRQ"),
                ColumnHeading::new("Idle"),
            ],
            CoreReading::table_record,
        )
    }

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
        TooltipBody::dictionary([KeyValue::new("CPU", percent_text(self.busy))])
    }
}
