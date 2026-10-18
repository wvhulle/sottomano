use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Frequency, frequency::gigahertz},
};
use vetrata_config::severity::Severity;
use vetrata_sources::sysfs_sensors::cpu_clock::{CoreFreq, CpuClock, FrequencyLimit};

use super::{
    chiplet_live_state::ChipletLiveState,
    core_grid_sections::{CoreGridSection, PerCoreReading},
};
use crate::widgets::{
    bar_widget::{BarWidget, Glyph},
    phosphor::Phosphor,
    tooltip_body::{
        ColumnHeading, KeyValue, Section, TableRecord, TableRow, ThreadCell, TooltipBody,
    },
    widget_state::WidgetState,
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
        let frequency = |key, frequency: Option<Frequency>| {
            frequency.map(|frequency| KeyValue::new(key, frequency_text(frequency)))
        };
        let capped = self.capped_cores();
        let capped_summary = (capped > 0).then(|| {
            KeyValue::new("capped", format!("{capped} of {} cores", self.cores.len()))
                .level(Severity::Warning)
        });
        TooltipBody::sections([Section::dictionary(
            [
                frequency("min", self.lowest()),
                frequency("avg", self.average()),
                frequency("max", self.highest()),
                frequency("cap", self.shared_ceiling()),
                capped_summary,
            ]
            .into_iter()
            .flatten(),
        )
        .titled("Frequency")])
    }
}

impl ChipletLiveState<'_> {
    pub fn clock_sections(&self, clock: &CpuClock) -> Vec<Section> {
        let grid = CoreGridSection {
            topology: &clock.topology,
            readings: &clock.cores,
        }
        .section(|group| self.title(group), |group, _| self.facts(group))
        .titled((
            "Chiplets".to_owned(),
            "Each square is one physical core in core-number order, not die position, split into \
             its hardware threads; bar height shows each thread's clock against the ceiling the \
             governor allows."
                .to_owned(),
        ));
        let capped: Vec<&CoreFreq> = clock
            .cores
            .iter()
            .filter(|core| core.limit.is_capped())
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

impl PerCoreReading for CoreFreq {
    fn core_index(&self) -> u32 {
        self.core
    }

    fn thread_cell(&self) -> ThreadCell {
        ThreadCell {
            fill: self.frequency / self.limit.ceiling(),
            level: limit_level(self.limit),
        }
    }
}

impl TableRow<2> for &CoreFreq {
    fn table_record(self) -> TableRecord<2> {
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
