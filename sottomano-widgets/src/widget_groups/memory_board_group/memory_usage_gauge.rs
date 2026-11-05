use sottomano_config::severity::Severity;
use sottomano_desktop_sources::cpu_memory_compute::{
    memory_module_temperatures::MemoryModuleTemperatures,
    memory_usage::{Memory, MemoryProcesses, Swap},
};
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
        "Memory"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Memory(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match self.percent() {
            p if p > 90 => ReadingDisplayState::Critical,
            p if p > 75 => ReadingDisplayState::Warning,
            _ => ReadingDisplayState::Idle,
        }
    }

    fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
        OngoingToastThreshold::NotifyFromSeverity {
            minimum_severity: Severity::Critical,
            summary_text: "Memory almost full",
        }
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(percent_fill(self.percent()))
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let swap_entries = match self.swap {
            Swap::NotConfigured => vec![LabeledValue::new("Swap", "none")],
            Swap::Configured { used, total } => vec![
                LabeledValue::new("Swap used", Quantity::size(used)),
                LabeledValue::new("Swap total", Quantity::size(total)),
            ],
        };
        HoverCardBody::dictionary(
            [
                LabeledValue::new("Memory", Quantity::share(percent_fill(self.percent()))),
                LabeledValue::new("Used", Quantity::size(self.used)),
                LabeledValue::new("Total", Quantity::size(self.total)),
            ]
            .into_iter()
            .chain(swap_entries),
        )
    }
}
