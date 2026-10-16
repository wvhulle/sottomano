use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Frequency, frequency::gigahertz},
};

use super::{
    chiplet_live_state::ChipletLiveState,
    core_grid_sections::{CoreGridSection, PerCore},
};
use crate::{
    data::{
        severity::Severity,
        source::sysfs_sensors::cpu_clock::{CoreFreq, CpuClock, FrequencyLimit},
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::Phosphor,
        tooltip_body::{ColumnHeading, KeyValue, Section, TableRecord, ThreadCell, TooltipBody},
        widget_state::WidgetState,
    },
};

pub(super) fn frequency_text(frequency: Frequency) -> String {
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
                frequency("cap", self.shared_ceiling()),
                self.capped_summary(),
            ]
            .into_iter()
            .flatten(),
        )
        .titled("Frequency")
    }

    fn shared_ceiling(&self) -> Option<Frequency> {
        let first = self.cores.first()?.limit.ceiling();
        self.cores
            .iter()
            .all(|core| core.limit.ceiling() == first)
            .then_some(first)
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
        Phosphor::WaveSine(self.state().weight()).into()
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
    pub fn core_sections(&self, live: &ChipletLiveState) -> Vec<Section> {
        let grid = CoreGridSection {
            topology: &self.topology,
            readings: &self.cores,
        }
        .section(|group| live.title(group), |group, _| live.facts(group))
        .titled((
            "Chiplets".to_owned(),
            "Each square is one physical core in core-number order, not die position, split into \
             its hardware threads; bar height shows each thread's clock against the ceiling the \
             governor allows."
                .to_owned(),
        ));
        let capped: Vec<TableRecord<2>> = self
            .cores
            .iter()
            .filter(|core| core.limit.is_capped())
            .map(CoreFreq::table_record)
            .collect();
        let capped_section = (!capped.is_empty()).then(|| {
            Section::column_table(
                [ColumnHeading::new("Clock"), ColumnHeading::new("Cap")],
                capped,
            )
            .titled("Capped below the hardware maximum")
        });
        [grid].into_iter().chain(capped_section).collect()
    }
}

impl PerCore for CoreFreq {
    fn core(&self) -> u32 {
        self.core
    }

    fn thread_cell(&self) -> ThreadCell {
        ThreadCell {
            fill: self.frequency / self.limit.ceiling(),
            level: limit_level(self.limit),
        }
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
