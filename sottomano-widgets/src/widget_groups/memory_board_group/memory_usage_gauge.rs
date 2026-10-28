use sottomano_config::severity::Severity;
use sottomano_sources::cpu_memory_compute::{
    memory_module_temperatures::MemoryModuleTemperatures,
    memory_usage::{Memory, MemoryProcesses},
};
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph, OngoingToastThreshold},
        gauge::percent_fill,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        tooltip_body::{ReadingSections, Section, TooltipBody},
        value_cells::LabeledValue,
        widget_state::WidgetState,
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

impl BarWidget for Memory {
    fn name(&self) -> &'static str {
        "Memory"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Memory(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        match self.percent() {
            p if p > 90 => WidgetState::Critical,
            p if p > 75 => WidgetState::Warning,
            _ => WidgetState::Idle,
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

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::dictionary([
            LabeledValue::new("Memory", Quantity::share(percent_fill(self.percent()))),
            LabeledValue::new("Used", Quantity::size(self.used)),
            LabeledValue::new("Total", Quantity::size(self.total)),
        ])
    }
}
