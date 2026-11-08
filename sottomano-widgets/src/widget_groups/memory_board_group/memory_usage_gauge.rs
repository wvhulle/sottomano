use sottomano_desktop_sources::cpu_memory_compute::{
    memory_module_temperatures::MemoryModuleTemperatures,
    memory_usage::{Memory, MemoryProcesses},
};
use sottomano_toml_config::severity::Severity;
use uom::si::f64::Ratio;

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
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

const FULL_WARNING_PERCENT: i64 = 75;
const FULL_CRITICAL_PERCENT: i64 = 90;

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

impl GroupItemReading for Memory {
    fn name(&self) -> &'static str {
        "RAM"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Memory(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match self.percent() {
            used if used > FULL_CRITICAL_PERCENT => Severity::Critical,
            used if used > FULL_WARNING_PERCENT => Severity::Warning,
            _ => Severity::Normal,
        }
        .into()
    }

    fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
        OngoingToastThreshold::NotifyFromSeverity {
            minimum_severity: Severity::Critical,
            summary_text: "RAM almost full",
        }
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(percent_fill(self.percent()))
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([
            LabeledValue::new("RAM used", Quantity::share(percent_fill(self.percent()))),
            LabeledValue::new("Used", Quantity::size(self.used)),
            LabeledValue::new("Total", Quantity::size(self.total)),
        ])
    }
}

#[cfg(test)]
mod tests {
    use bytesize::ByteSize;

    use super::*;
    use crate::widgets::ongoing_toast::reading_display_state_alert_severity;

    fn memory_at(used_gibibytes: u64) -> Memory {
        Memory {
            used: ByteSize::gib(used_gibibytes),
            total: ByteSize::gib(100),
        }
    }

    #[test]
    fn fullness_sets_the_state() {
        assert_eq!(
            memory_at(40).state(Clock::real()),
            ReadingDisplayState::Idle
        );
        assert_eq!(
            memory_at(80).state(Clock::real()),
            ReadingDisplayState::Warning
        );
        assert_eq!(
            memory_at(95).state(Clock::real()),
            ReadingDisplayState::Critical
        );
    }

    #[test]
    fn only_a_nearly_full_ram_raises_a_toast() {
        assert_eq!(
            reading_display_state_alert_severity(&memory_at(80), Clock::real()),
            None
        );
        assert_eq!(
            reading_display_state_alert_severity(&memory_at(95), Clock::real()),
            Some(Severity::Critical)
        );
    }
}
