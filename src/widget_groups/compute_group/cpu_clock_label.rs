use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Frequency, frequency::gigahertz},
};

use super::core_group_sections::{PerCore, sections_by_core_group};
use crate::{
    data::{
        severity::Severity,
        source::sysfs_sensors::cpu_clock::{CoreFreq, CpuClock, FrequencyLimit},
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::Phosphor,
        tooltip_body::{ColumnHeading, KeyValue, Section, TableRecord, TooltipBody},
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
        let frequency = |key, frequency: Option<Frequency>| {
            frequency.map(|frequency| KeyValue::new(key, frequency_text(frequency)))
        };
        Section::dictionary(
            [
                frequency("min", self.lowest()),
                frequency("avg", self.average()),
                frequency("max", self.highest()),
                self.capped_summary(),
            ]
            .into_iter()
            .flatten(),
        )
        .titled("Frequency")
    }

    fn capped_summary(&self) -> Option<KeyValue> {
        let capped = self.capped_cores();
        (capped > 0).then(|| {
            KeyValue::new("capped", format!("{capped} of {} cores", self.cores.len()))
                .level(Severity::Warning)
        })
    }
}

impl BarWidget for CpuClock {
    fn name(&self) -> &'static str {
        "CPU clock"
    }

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
        TooltipBody::sections([self.frequency_section()])
    }
}

impl CpuClock {
    pub fn core_sections(&self) -> Vec<Section> {
        sections_by_core_group(
            &self.topology,
            &self.cores,
            [ColumnHeading::new("Clock"), ColumnHeading::new("Cap")],
            "Clock is the current speed; Cap is the ceiling the governor allows. A cap below the \
             hardware maximum is marked.",
            CoreFreq::table_record,
        )
    }
}

impl PerCore for CoreFreq {
    fn core(&self) -> u32 {
        self.core
    }
}

impl CoreFreq {
    fn table_record(&self) -> TableRecord<2> {
        TableRecord::new(
            format!("Core {}", self.core),
            [
                Some(frequency_text(self.frequency).into()),
                Some(frequency_text(self.limit.ceiling()).into()),
            ],
        )
        .level(limit_level(self.limit))
    }
}
