use std::time::Duration;

use chrono::Locale;
use quick_xml::escape::escape;
use sottomano_desktop_sources::cpu_memory_compute::{
    memory_module_temperatures::MemoryModuleTemperatures,
    memory_usage::{MemoryProcesses, MemoryTrend, Swap},
};
use sottomano_toml_config::severity::Severity;
use uom::si::{f64::Ratio, ratio::percent};

use crate::{
    clock::Clock,
    group_items::{
        gauge::percent_fill,
        group_item_reading::{GroupItemReading, OngoingToastThreshold},
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, ReadingSections, Section},
    widgets::{
        icon_glyph::IconGlyph,
        localized_durations::duration_text,
        localized_quantities::Quantity,
        ongoing_toast::definition_list,
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
};

const FULL_WARNING_PERCENT: i64 = 75;
const FULL_CRITICAL_PERCENT: i64 = 90;
const STALL_WARNING_PERCENT: f64 = 20.0;
const STALL_CRITICAL_PERCENT: f64 = 40.0;
const EXHAUSTION_WARNING: Duration = Duration::from_secs(300);
const EXHAUSTION_CRITICAL: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemoryAlarmCause {
    FillingFast,
    UnderPressure,
    AlmostFull,
}

impl MemoryAlarmCause {
    const fn summary_text(self) -> &'static str {
        match self {
            MemoryAlarmCause::FillingFast => "RAM filling fast",
            MemoryAlarmCause::UnderPressure => "RAM under pressure",
            MemoryAlarmCause::AlmostFull => "RAM almost full",
        }
    }
}

fn exhaustion_severity(memory: &MemoryTrend) -> Severity {
    match memory.exhaustion_projection {
        Some(remaining) if remaining < EXHAUSTION_CRITICAL => Severity::Critical,
        Some(remaining) if remaining < EXHAUSTION_WARNING => Severity::Warning,
        _ => Severity::Normal,
    }
}

fn stall_severity(memory: &MemoryTrend) -> Severity {
    match memory.stall_share.map(|share| share.get::<percent>()) {
        Some(stalled) if stalled >= STALL_CRITICAL_PERCENT => Severity::Critical,
        Some(stalled) if stalled >= STALL_WARNING_PERCENT => Severity::Warning,
        _ => Severity::Normal,
    }
}

fn fullness_severity(memory: &MemoryTrend) -> Severity {
    match memory.usage.percent() {
        used if used > FULL_CRITICAL_PERCENT => Severity::Critical,
        used if used > FULL_WARNING_PERCENT => Severity::Warning,
        _ => Severity::Normal,
    }
}

fn memory_alarm(memory: &MemoryTrend) -> Option<(Severity, MemoryAlarmCause)> {
    [
        (exhaustion_severity(memory), MemoryAlarmCause::FillingFast),
        (stall_severity(memory), MemoryAlarmCause::UnderPressure),
        (fullness_severity(memory), MemoryAlarmCause::AlmostFull),
    ]
    .into_iter()
    .filter(|(severity, _)| *severity > Severity::Normal)
    .reduce(|kept, candidate| {
        if candidate.0 > kept.0 {
            candidate
        } else {
            kept
        }
    })
}

impl ReadingSections for MemoryModuleTemperatures {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::dictionary(self.modules.iter().map(|module| {
            LabeledValue::new(
                module.bus_address.clone(),
                Quantity::temperature(module.temperature),
            )
        }))]
    }
}

impl ReadingSections for MemoryProcesses {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::dictionary(self.processes.iter().map(|process| {
            LabeledValue::new(process.name.clone(), Quantity::size(process.resident))
        }))]
    }
}

impl GroupItemReading for MemoryTrend {
    fn name(&self) -> &'static str {
        "RAM"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Memory(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match memory_alarm(self) {
            Some((Severity::Critical, _)) => ReadingDisplayState::Critical,
            Some(_) => ReadingDisplayState::Warning,
            None => ReadingDisplayState::Idle,
        }
    }

    fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
        let cause = memory_alarm(self).map_or(MemoryAlarmCause::AlmostFull, |(_, cause)| cause);
        OngoingToastThreshold::NotifyFromSeverity {
            minimum_severity: Severity::Warning,
            summary_text: cause.summary_text(),
        }
    }

    fn ongoing_toast_body(&self, _clock: Clock, locale: Locale) -> Option<String> {
        let exhaustion_row = self
            .exhaustion_projection
            .filter(|remaining| *remaining < EXHAUSTION_WARNING)
            .map(|remaining| ("Full in".to_owned(), duration_text(remaining, locale)));
        let grower_rows = self.growing_processes.iter().map(|process| {
            (
                escape(process.name.as_str()).into_owned(),
                format!(
                    "+{}/min",
                    Quantity::size(process.growth_per_minute).localized_text(locale)
                ),
            )
        });
        let body_rows: Vec<(String, String)> =
            exhaustion_row.into_iter().chain(grower_rows).collect();
        (!body_rows.is_empty()).then(|| {
            definition_list(
                body_rows
                    .iter()
                    .map(|(row_key, row_value)| (row_key.as_str(), row_value.clone())),
            )
        })
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(percent_fill(self.usage.percent()))
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let swap_entries = match self.usage.swap {
            Swap::NotConfigured => vec![LabeledValue::new("Swap", "none")],
            Swap::Configured { used, total } => vec![
                LabeledValue::new("Swap used", Quantity::size(used)),
                LabeledValue::new("Swap total", Quantity::size(total)),
            ],
        };
        let trend_entries = self
            .exhaustion_projection
            .filter(|remaining| *remaining < EXHAUSTION_WARNING)
            .map(|remaining| LabeledValue::new("Full in", ValueCell::duration(remaining)))
            .into_iter()
            .chain(
                self.stall_share
                    .map(|share| LabeledValue::new("Stalled last minute", Quantity::share(share))),
            );
        HoverCardBody::dictionary(
            [
                LabeledValue::new(
                    "RAM used",
                    Quantity::share(percent_fill(self.usage.percent())),
                ),
                LabeledValue::new("Used", Quantity::size(self.usage.used)),
                LabeledValue::new("Total", Quantity::size(self.usage.total)),
            ]
            .into_iter()
            .chain(trend_entries)
            .chain(swap_entries),
        )
    }
}

#[cfg(test)]
mod tests {
    use bytesize::ByteSize;
    use sottomano_desktop_sources::cpu_memory_compute::memory_usage::{
        GrowingMemoryProcess, Memory,
    };

    use super::*;

    fn memory_at(used_gibibytes: u64) -> MemoryTrend {
        MemoryTrend {
            usage: Memory {
                used: ByteSize::gib(used_gibibytes),
                total: ByteSize::gib(100),
                swap: Swap::NotConfigured,
            },
            stall_share: None,
            exhaustion_projection: None,
            growing_processes: Vec::new(),
        }
    }

    fn summary_of(memory: &MemoryTrend) -> &'static str {
        match memory.ongoing_toast_threshold() {
            OngoingToastThreshold::NotifyFromSeverity { summary_text, .. } => summary_text,
            OngoingToastThreshold::NeverNotify => "",
        }
    }

    #[test]
    fn calm_memory_is_idle() {
        assert_eq!(
            memory_at(40).state(Clock::real()),
            ReadingDisplayState::Idle
        );
    }

    #[test]
    fn chronic_swap_stalls_stay_quiet() {
        let swapping = MemoryTrend {
            stall_share: Some(Ratio::new::<percent>(7.0)),
            ..memory_at(65)
        };
        assert_eq!(swapping.state(Clock::real()), ReadingDisplayState::Idle);
    }

    #[test]
    fn each_cause_raises_the_state_on_its_own() {
        let nearly_full = memory_at(95);
        let stalled = MemoryTrend {
            stall_share: Some(Ratio::new::<percent>(25.0)),
            ..memory_at(40)
        };
        let filling = MemoryTrend {
            exhaustion_projection: Some(Duration::from_secs(30)),
            ..memory_at(40)
        };
        assert_eq!(
            nearly_full.state(Clock::real()),
            ReadingDisplayState::Critical
        );
        assert_eq!(stalled.state(Clock::real()), ReadingDisplayState::Warning);
        assert_eq!(filling.state(Clock::real()), ReadingDisplayState::Critical);
        assert_eq!(summary_of(&nearly_full), "RAM almost full");
        assert_eq!(summary_of(&stalled), "RAM under pressure");
        assert_eq!(summary_of(&filling), "RAM filling fast");
    }

    #[test]
    fn the_most_severe_cause_names_the_toast() {
        let memory = MemoryTrend {
            stall_share: Some(Ratio::new::<percent>(50.0)),
            exhaustion_projection: Some(Duration::from_secs(200)),
            ..memory_at(40)
        };
        assert_eq!(summary_of(&memory), "RAM under pressure");
    }

    #[test]
    fn toast_body_names_the_growing_processes() {
        let memory = MemoryTrend {
            exhaustion_projection: Some(Duration::from_secs(40)),
            growing_processes: vec![GrowingMemoryProcess {
                pid: 4,
                name: "firefox".to_owned(),
                resident: ByteSize::gib(6),
                growth_per_minute: ByteSize::gib(2),
            }],
            ..memory_at(60)
        };
        let body = memory
            .ongoing_toast_body(Clock::real(), Locale::en_US)
            .expect("an alarming reading has a body");
        assert!(body.starts_with("<dl><dt>Full in</dt><dd>"), "{body}");
        assert!(body.contains("<dt>firefox</dt><dd>+"), "{body}");
        assert!(!body.contains('·'), "{body}");
    }
}
