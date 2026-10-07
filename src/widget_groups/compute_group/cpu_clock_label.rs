use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Frequency, frequency::gigahertz},
};

use super::core_kind_sections::{PerCore, sections_by_core_kind};
use crate::{
    data::{
        severity::Severity,
        source::sysfs_sensors::cpu_clock::{CoreFreq, CpuClock, FrequencyLimit},
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::Phosphor,
        tooltip_body::{Cell, Row, TableRow, TooltipBody, row, section},
        widget_state::WidgetState,
    },
};

fn frequency_text(frequency: Frequency) -> String {
    format!("{:.2}", frequency.into_format_args(gigahertz, Abbreviation))
}

fn limit_level(limit: FrequencyLimit) -> Severity {
    match limit {
        FrequencyLimit::Unrestricted(_) => Severity::Normal,
        FrequencyLimit::Capped(_) => Severity::Warning,
    }
}

impl CpuClock {
    fn summary_rows(&self) -> Vec<Row> {
        [
            self.lowest()
                .map(|lowest| row("min", frequency_text(lowest))),
            self.average()
                .map(|average| row("avg", frequency_text(average))),
            self.highest()
                .map(|highest| row("max", frequency_text(highest))),
            self.capped_summary(),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    fn capped_summary(&self) -> Option<Row> {
        let capped = self.capped_cores();
        (capped > 0).then(|| Row {
            level: Severity::Warning,
            ..row("capped", format!("{capped} of {} cores", self.cores.len()))
        })
    }
}

impl BarWidget for CpuClock {
    fn icon(&self) -> Glyph {
        Phosphor::Gauge(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        if self.capped_cores() > 0 {
            WidgetState::Warning
        } else {
            WidgetState::Idle
        }
    }

    fn label(&self) -> Option<String> {
        self.average().map(frequency_text)
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Sections(
            [section("Frequency", self.summary_rows())]
                .into_iter()
                .chain(sections_by_core_kind(&self.topology, self.cores.clone()))
                .collect(),
        )
    }
}

impl PerCore for CoreFreq {
    fn core(&self) -> u32 {
        self.core
    }
}

impl TableRow for CoreFreq {
    const HEADINGS: &'static [&'static str] = &["Clock", "Cap"];

    fn key(&self) -> Cell {
        format!("Core {}", self.core).into()
    }

    fn cells(&self) -> Vec<Cell> {
        vec![
            frequency_text(self.frequency).into(),
            frequency_text(self.limit.ceiling()).into(),
        ]
    }

    fn level(&self) -> Severity {
        limit_level(self.limit)
    }
}
