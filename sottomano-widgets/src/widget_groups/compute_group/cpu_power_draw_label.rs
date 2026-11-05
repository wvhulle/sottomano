use chrono::Locale;
use sottomano_desktop_sources::cpu_memory_compute::cpu_power_draw::CpuPower;
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Power, power::watt},
};

use crate::{
    clock::Clock,
    group_items::group_item_reading::GroupItemReading,
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
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

impl GroupItemReading for CpuPower {
    fn visible_on_panel(&self) -> bool {
        package_draw(self).is_some()
    }

    fn name(&self) -> &'static str {
        "CPU power draw"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Lightning(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        package_draw(self).map(watt_text)
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([Section::dictionary(
            self.zones
                .iter()
                .map(|zone| LabeledValue::new(zone.name.clone(), Quantity::power(zone.draw))),
        )
        .titled((
            "CPU power draw per RAPL zone".to_owned(),
            "package covers the whole CPU die, core and uncore are parts of it, dram is the \
             memory controller and psys the whole platform where the firmware reports it."
                .to_owned(),
        ))])
    }
}
