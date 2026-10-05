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
        tooltip_body::{Cell, TableRow, TooltipBody, row, untitled},
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

impl TableRow for CoreReading {
    const HEADINGS: &'static [&'static str] = &["Load", "User", "Sys", "IO", "IRQ", "Idle"];

    fn key(&self) -> Cell {
        format!("Core {}", self.core).into()
    }

    fn cells(&self) -> Vec<Cell> {
        [
            self.busy,
            self.split.user,
            self.split.system,
            self.split.iowait,
            self.split.irq,
        ]
        .into_iter()
        .chain(self.deep_idle)
        .map(|share| percent_text(share).into())
        .collect()
    }

    fn level(&self) -> Severity {
        busy_level(self.busy)
    }
}

impl BarWidget for CpuUsage {
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
        TooltipBody::Sections(
            [untitled(vec![row("CPU", percent_text(self.busy))])]
                .into_iter()
                .chain(sections_by_core_kind(&self.topology, self.core_readings()))
                .collect(),
        )
    }
}
