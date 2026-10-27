use chrono::Locale;
use sottomano_sources::cpu_memory_compute::cpu_power_draw::CpuPower;
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Power, power::watt},
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

fn watt_text(power: Power) -> String {
    format!("{:.1}", power.into_format_args(watt, Abbreviation))
}

fn package_draw(power: &CpuPower) -> Option<Power> {
    power
        .zones
        .iter()
        .filter(|zone| zone.name.starts_with("package"))
        .map(|zone| zone.draw)
        .reduce(|total, draw| total + draw)
}

impl BarWidget for CpuPower {
    fn visible_in_bar(&self) -> bool {
        package_draw(self).is_some()
    }

    fn name(&self) -> &'static str {
        "CPU power draw"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Lightning(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        package_draw(self).map(watt_text)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([Section::dictionary(
            self.zones
                .iter()
                .map(|zone| KeyValue::new(zone.name.clone(), Quantity::power(zone.draw))),
        )
        .titled((
            "CPU power draw per RAPL zone".to_owned(),
            "package covers the whole CPU die, core and uncore are parts of it, dram is the \
             memory controller and psys the whole platform where the firmware reports it."
                .to_owned(),
        ))])
    }
}
