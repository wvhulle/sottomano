use std::num::NonZeroUsize;

use bytesize::ByteSize;
use chrono::Locale;
use sottomano_desktop_sources::{
    SnapshotSource,
    cpu_memory_compute::memory_usage::{Memory, MemoryProcesses},
};
use sottomano_toml_config::severity::Severity;

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        gauge::percent_fill,
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::Quantity,
        ongoing_toast::{
            IndicatorAlerts, NotificationBody, NotificationProgress, StateToast, definition_list,
        },
        value_cells::LabeledValue,
    },
};

const FULL_WARNING_PERCENT: i64 = 75;
const LISTED_PROCESSES: NonZeroUsize = NonZeroUsize::new(10).unwrap();
const FULL_CRITICAL_PERCENT: i64 = 90;

pub fn memory_processes_sections(memory_processes: &MemoryProcesses) -> Vec<Section> {
    vec![Section::new(
        "Resident memory",
        dictionary_table(memory_processes.processes.iter().map(|process| {
            LabeledValue::new(process.name.clone(), Quantity::size(process.resident))
        }))
        .truncated_to(LISTED_PROCESSES),
    )]
}

pub fn memory_indicator(memory: &Memory, locale: Locale) -> GroupIndicator<'_> {
    let state = match memory.percent() {
        used if used > FULL_CRITICAL_PERCENT => Severity::Critical,
        used if used > FULL_WARNING_PERCENT => Severity::Warning,
        _ => Severity::Normal,
    }
    .into();
    let fill = percent_fill(memory.percent());
    let available_bytes = memory
        .total
        .as_u64()
        .checked_sub(memory.used.as_u64())
        .expect("used memory is derived as MemTotal minus MemAvailable");
    let toast_body = definition_list([
        ("Used", Quantity::size(memory.used).localized_text(locale)),
        (
            "Available",
            Quantity::size(ByteSize::b(available_bytes)).localized_text(locale),
        ),
    ]);
    GroupIndicator::glyph_only(
        Memory::NAME,
        Phosphor::Memory(Weight::Regular).into(),
        state,
    )
    .with_readout(IndicatorReadout {
        label: ReadoutLabel::Unlabeled,
        gauge: ReadoutGauge::Gauged { fill, live: fill },
        badge: ReadoutBadge::Unbadged,
    })
    .with_alerts(IndicatorAlerts {
        state_toast: StateToast::NotifyFromSeverity {
            minimum_severity: Severity::Critical,
            summary_text: "RAM almost full",
            body: NotificationBody::Text(toast_body),
            progress: NotificationProgress::Measured(fill),
        },
        ..IndicatorAlerts::default()
    })
    .with_hover_card(Box::new(move |_| memory_hover_card(memory)))
}

pub fn memory_hover_card(memory: &Memory) -> Vec<Section> {
    vec![Section::new(
        "Usage",
        dictionary_table([
            LabeledValue::new("RAM used", Quantity::share(percent_fill(memory.percent()))),
            LabeledValue::new("Used", Quantity::size(memory.used)),
            LabeledValue::new("Total", Quantity::size(memory.total)),
        ]),
    )]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group_indicators::{
        group_indicator::indicator_toast_severity, indicator_state::IndicatorState,
    };

    fn memory_at(used_gibibytes: u64) -> Memory {
        Memory {
            used: ByteSize::gib(used_gibibytes),
            total: ByteSize::gib(100),
        }
    }

    #[test]
    fn fullness_sets_the_state() {
        assert_eq!(
            memory_indicator(&memory_at(40), Locale::default()).shown_state(),
            IndicatorState::Idle
        );
        assert_eq!(
            memory_indicator(&memory_at(80), Locale::default()).shown_state(),
            IndicatorState::Warning
        );
        assert_eq!(
            memory_indicator(&memory_at(95), Locale::default()).shown_state(),
            IndicatorState::Critical
        );
    }

    #[test]
    fn only_a_nearly_full_ram_raises_a_toast() {
        assert_eq!(
            indicator_toast_severity(&memory_indicator(&memory_at(80), Locale::default())),
            None
        );
        assert_eq!(
            indicator_toast_severity(&memory_indicator(&memory_at(95), Locale::default())),
            Some(Severity::Critical)
        );
    }

    #[test]
    fn ram_toast_lists_used_and_available_memory() {
        let nearly_full = memory_at(95);
        let toast_indicator = memory_indicator(&nearly_full, Locale::en_US);
        let StateToast::NotifyFromSeverity {
            body: NotificationBody::Text(toast_body),
            ..
        } = toast_indicator.alerts.state_toast.clone()
        else {
            panic!("a nearly full RAM has a toast with a body");
        };
        let used_text = Quantity::size(ByteSize::gib(95)).localized_text(Locale::en_US);
        let available_text = Quantity::size(ByteSize::gib(5)).localized_text(Locale::en_US);
        assert_eq!(
            toast_body,
            format!(
                "<dl><dt>Used</dt><dd>{used_text}</dd><dt>Available</dt><dd>{available_text}</\
                 dd></dl>"
            )
        );
    }
}
