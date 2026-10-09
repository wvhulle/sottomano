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
        tooltip_body::{Cell, Row, Section, TableRow, TooltipBody},
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
    fn frequency_section(&self) -> Section {
        Section::titled(
            "Frequency",
            &[
                ("min", self.lowest().map(frequency_text)).into(),
                ("avg", self.average().map(frequency_text)).into(),
                ("max", self.highest().map(frequency_text)).into(),
                self.capped_summary(),
            ],
        )
    }

    fn capped_summary(&self) -> Row {
        let capped = self.capped_cores();
        Row::new(
            "capped",
            &[(capped > 0)
                .then(|| format!("{capped} of {} cores", self.cores.len()))
                .into()],
            Severity::Warning,
        )
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
        TooltipBody::sections(
            &[
                vec![self.frequency_section()],
                sections_by_core_kind(&self.topology, self.cores.clone()),
            ]
            .concat(),
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
