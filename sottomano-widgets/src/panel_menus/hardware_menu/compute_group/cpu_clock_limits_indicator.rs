use std::cmp::Reverse;

use sottomano_desktop_sources::cpu_memory_compute::{
    cpu_clock_frequency::{ClockLimitKind, CpuClock},
    kernel_frequency_limit_causes::FrequencyLimitCause,
};
use sottomano_toml_config::severity::Severity;

use super::{ComputeGroup, ComputeInteraction, ComputeSource, core_grid_sections::cpu_ranges};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_group_content::IndicatorListing,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::value_cells::LabeledValue,
};

pub(super) fn cause_text(cause: FrequencyLimitCause) -> String {
    match cause {
        FrequencyLimitCause::BoostDisabled => "boost off".to_owned(),
        FrequencyLimitCause::PolicyCap => "policy cap".to_owned(),
        FrequencyLimitCause::KernelThermalCooling { level, deepest } => {
            format!("kernel cooling {level}/{deepest}")
        }
    }
}

pub(super) fn cause_level(cause: FrequencyLimitCause) -> Severity {
    match cause {
        FrequencyLimitCause::KernelThermalCooling { .. } => Severity::Critical,
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

pub struct ClockLimitCauses<'view> {
    pub clock: &'view CpuClock,
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

impl<'view> ClockLimitCauses<'view> {
    pub fn panel_visibility(&self) -> PanelVisibility {
        if self.clock.limited_cores() > 0 {
            PanelVisibility::Shown
        } else {
            PanelVisibility::Hidden
        }
    }

    pub fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Published limits",
            dictionary_table(self.spreads().iter().map(|spread| {
                LabeledValue::new(cause_text(spread.cause), self.spread_text(spread))
                    .level(cause_level(spread.cause))
            })),
        )]
    }

    pub fn indicator(self) -> GroupIndicator<'view> {
        let spreads = self.spreads();
        let label = match spreads.first() {
            Some(most_severe) => ReadoutLabel::Labeled(match spreads.len() - 1 {
                0 => cause_text(most_severe.cause),
                others => format!("{} +{others}", cause_text(most_severe.cause)),
            }),
            None => ReadoutLabel::Unlabeled,
        };
        GroupIndicator::glyph_only(
            "CPU clock limits",
            Phosphor::Gauge(Weight::Regular).into(),
            self.severity().into(),
        )
        .with_readout(IndicatorReadout {
            label,
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_panel_visibility(self.panel_visibility())
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }
}

impl ComputeGroup {
    pub(super) fn cpu_clock_limits_listing(&self) -> IndicatorListing<'_> {
        IndicatorListing::Listed(
            self.cpu_clock
                .get()
                .map(|held_snapshot| {
                    ClockLimitCauses {
                        clock: held_snapshot,
                    }
                    .indicator()
                })
                .into_iter()
                .flat_map(GroupIndicator::when_shown)
                .map(|limits_row| self.wired_cpu_clock_limits_indicator(limits_row))
                .collect(),
        )
    }

    fn wired_cpu_clock_limits_indicator<'view>(
        &'view self,
        limits_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        limits_indicator
            .with_hover_card(Box::new(move |_| self.cpu_clock_limits_card()))
            .refreshed_by(ComputeInteraction::Refresh(ComputeSource::CpuClock))
    }

    fn cpu_clock_limits_card(&self) -> Vec<Section> {
        let Some(cpu_clock) = self.cpu_clock.get() else {
            return Vec::new();
        };
        let limits = ClockLimitCauses { clock: cpu_clock };
        if limits.panel_visibility() != PanelVisibility::Shown {
            return Vec::new();
        }
        self.cpu_clock.hover_card_with(limits.hover_card())
    }
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::cpu_memory_compute::{
        cpu_topology::CpuTopology, intel_thermal_throttle_counters::ThermalThrottleActivity,
        kernel_frequency_limit_causes::CoreLimitCauses,
    };

    use super::*;
    use crate::group_indicators::group_indicator::PanelVisibility;

    fn clock_with(causes: [Vec<FrequencyLimitCause>; 4]) -> CpuClock {
        CpuClock {
            cores: Vec::new(),
            limit_causes: causes
                .into_iter()
                .zip(0..)
                .map(|(causes, core)| CoreLimitCauses { core, causes })
                .collect(),
            thermal_throttle_activity: ThermalThrottleActivity::Quiet,
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
