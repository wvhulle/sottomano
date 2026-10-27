use chrono::Locale;
use sottomano_sources::cpu_memory_compute::{
    cpu_clock_frequency::CpuClock, cpu_core_loads::CpuCoreLoads,
};
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Frequency, frequency::gigahertz},
};

use super::{
    chiplet_clock_table::ChipletClockTable, chiplet_live_state::ChipletLiveState,
    cpu_clock_limit_causes::causes_text,
};
use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, Section, TooltipBody},
    },
};

fn frequency_text(frequency: Frequency) -> String {
    format!("{:.2}", frequency.into_format_args(gigahertz, Abbreviation))
}

impl BarWidget for CpuClock {
    fn name(&self) -> &'static str {
        "CPU clock"
    }

    fn icon(&self) -> Glyph {
        Phosphor::WaveSine(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        self.average().map(frequency_text)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        let frequency = |key, frequency: Option<Frequency>| {
            frequency.map(|frequency| KeyValue::new(key, Quantity::clock_gigahertz(frequency)))
        };
        let shared_causes = self.shared_causes();
        TooltipBody::sections([Section::dictionary(
            [
                frequency("min", self.lowest()),
                frequency("avg", self.average()),
                frequency("max", self.highest()),
                frequency("floor", self.shared_floor()),
                frequency("cap", self.shared_ceiling()),
                frequency("max hw", self.shared_hardware_ceiling()),
                causes_text(&shared_causes).map(|text| KeyValue::new("limited by", text)),
            ]
            .into_iter()
            .flatten(),
        )
        .titled("Frequency")])
    }
}

impl ChipletLiveState<'_> {
    pub fn clock_sections(&self, clock: &CpuClock, loads: Option<&CpuCoreLoads>) -> Vec<Section> {
        clock
            .topology
            .core_groups()
            .iter()
            .enumerate()
            .map(|(position, group)| {
                let title = match self.temperature_text(group) {
                    Some(temperature) => format!("{} · {temperature}", self.title(group)),
                    None => self.title(group),
                };
                let table = ChipletClockTable {
                    clock,
                    loads,
                    group,
                }
                .section();
                if position == 0 {
                    table.titled((
                        title,
                        "Cap appears only where a core's policy ceiling differs from the shared \
                         cap above. Limit lists causes the kernel publishes for that core alone; \
                         causes every CPU shares are listed once above. Firmware power and \
                         current limits are not published on every CPU, so a busy core below its \
                         cap with no listed cause is held back by something the kernel cannot see."
                            .to_owned(),
                    ))
                } else {
                    table.titled(title)
                }
            })
            .collect()
    }
}
