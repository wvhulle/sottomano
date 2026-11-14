use std::{borrow::Cow, cmp::Reverse};

use chrono::Locale;
use sottomano_desktop_sources::cpu_memory_compute::{
    cpu_clock_frequency::{ClockLimitKind, CpuClock},
    kernel_frequency_limit_causes::FrequencyLimitCause,
};
use sottomano_toml_config::severity::Severity;

use super::core_grid_sections::cpu_ranges;
use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::{GroupItemReading, PanelVisibility},
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

pub(super) fn cause_text(cause: FrequencyLimitCause) -> String {
    match cause {
        FrequencyLimitCause::BoostDisabled => "boost off".to_owned(),
        FrequencyLimitCause::PolicyCap => "policy cap".to_owned(),
        FrequencyLimitCause::KernelThermalCooling { level, deepest } => {
            format!("kernel cooling {level}/{deepest}")
        }
        FrequencyLimitCause::CoreThermalThrottle { events } => {
            format!("core thermal throttle ×{events}")
        }
        FrequencyLimitCause::PackageThermalThrottle { events } => {
            format!("package thermal throttle ×{events}")
        }
    }
}

pub(super) fn cause_level(cause: FrequencyLimitCause) -> Severity {
    match cause {
        FrequencyLimitCause::KernelThermalCooling { .. }
        | FrequencyLimitCause::CoreThermalThrottle { .. }
        | FrequencyLimitCause::PackageThermalThrottle { .. } => Severity::Critical,
        FrequencyLimitCause::BoostDisabled | FrequencyLimitCause::PolicyCap => Severity::Warning,
    }
}

pub(super) fn causes_text(causes: &[FrequencyLimitCause]) -> Option<String> {
    (!causes.is_empty()).then(|| {
        causes
            .iter()
            .map(|cause| cause_text(*cause))
            .collect::<Vec<_>>()
            .join(", ")
    })
}

struct CauseSpread {
    cause: FrequencyLimitCause,
    cpus: Vec<u32>,
}

pub struct ClockLimitCauses<'a> {
    pub clock: &'a CpuClock,
}

impl ClockLimitCauses<'_> {
    fn spreads(&self) -> Vec<CauseSpread> {
        let mut spreads = self
            .clock
            .limit_causes
            .iter()
            .flat_map(|core| core.causes.iter().map(|cause| (core.core, *cause)))
            .fold(Vec::<CauseSpread>::new(), |mut spreads, (cpu, cause)| {
                match spreads.iter_mut().find(|spread| spread.cause == cause) {
                    Some(spread) => spread.cpus.push(cpu),
                    None => spreads.push(CauseSpread {
                        cause,
                        cpus: vec![cpu],
                    }),
                }
                spreads
            });
        spreads.sort_by_key(|spread| Reverse(cause_level(spread.cause)));
        spreads
    }

    fn spread_text(&self, spread: &CauseSpread) -> String {
        if spread.cpus.len() == self.clock.limit_causes.len() {
            "all CPUs".to_owned()
        } else {
            format!("CPUs {}", cpu_ranges(spread.cpus.iter().copied()))
        }
    }

    fn severity(&self) -> Severity {
        match self.clock.limit_kind() {
            ClockLimitKind::Thermal => Severity::Critical,
            ClockLimitKind::NonThermal => Severity::Warning,
        }
    }
}

impl GroupItemReading for ClockLimitCauses<'_> {
    fn name(&self) -> Cow<'static, str> {
        "CPU clock limits".into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Gauge(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        self.severity().into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        let spreads = self.spreads();
        let most_severe = spreads.first()?;
        Some(match spreads.len() - 1 {
            0 => cause_text(most_severe.cause),
            others => format!("{} +{others}", cause_text(most_severe.cause)),
        })
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([Section::dictionary(self.spreads().iter().map(|spread| {
            LabeledValue::new(cause_text(spread.cause), self.spread_text(spread))
                .level(cause_level(spread.cause))
        }))
        .titled("Published limits")])
    }

    fn panel_visibility(&self) -> PanelVisibility {
        if self.clock.limited_cores() > 0 {
            PanelVisibility::Shown
        } else {
            PanelVisibility::Hidden
        }
    }
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::cpu_memory_compute::{
        cpu_topology::CpuTopology, kernel_frequency_limit_causes::CoreLimitCauses,
    };

    use super::*;
    use crate::group_items::group_item_reading::PanelVisibility;

    fn clock_with(causes: [Vec<FrequencyLimitCause>; 4]) -> CpuClock {
        CpuClock {
            cores: Vec::new(),
            limit_causes: causes
                .into_iter()
                .zip(0..)
                .map(|(causes, core)| CoreLimitCauses { core, causes })
                .collect(),
            topology: CpuTopology::default(),
        }
    }

    #[test]
    fn hidden_without_published_causes() {
        let clock = clock_with([Vec::new(), Vec::new(), Vec::new(), Vec::new()]);
        assert_eq!(
            ClockLimitCauses { clock: &clock }.panel_visibility(),
            PanelVisibility::Hidden
        );
    }

    #[test]
    fn causes_on_every_cpu_read_as_all_cpus_and_partial_ones_as_ranges() {
        let boost = FrequencyLimitCause::BoostDisabled;
        let cooling = FrequencyLimitCause::KernelThermalCooling {
            level: 1,
            deepest: 3,
        };
        let clock = clock_with([
            vec![boost],
            vec![boost, cooling],
            vec![boost, cooling],
            vec![boost],
        ]);
        let limits = ClockLimitCauses { clock: &clock };
        let texts: Vec<String> = limits
            .spreads()
            .iter()
            .map(|spread| {
                format!(
                    "{}: {}",
                    cause_text(spread.cause),
                    limits.spread_text(spread)
                )
            })
            .collect();
        assert_eq!(
            texts,
            ["kernel cooling 1/3: CPUs 1–2", "boost off: all CPUs"]
        );
    }
}
