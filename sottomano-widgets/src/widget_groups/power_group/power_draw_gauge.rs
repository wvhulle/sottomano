use sottomano_desktop_sources::battery_power_profiles::upower_battery_state::{
    Battery, ChargeStatus, ChargerSupply,
};
use uom::si::{f64::Power, power::watt};

use crate::{
    clock::Clock,
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{ReadingSections, Section, SectionTable, dictionary_table},
    widgets::{
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

const POWER_WARN_W: f64 = 10.0;
const POWER_CRITICAL_W: f64 = 20.0;
const POWER_GAUGE_MAX_W: f64 = 40.0;

pub struct PowerDraw {
    power: Power,
    status: ChargeStatus,
    expected_laptop: Option<Power>,
}

impl PowerDraw {
    pub fn battery_draw(battery: &Battery, expected_laptop: Option<Power>) -> Self {
        PowerDraw {
            power: battery.power,
            status: battery.status,
            expected_laptop,
        }
    }

    pub fn power_source(&self) -> PowerSource {
        if matches!(
            self.status,
            ChargeStatus::Discharging | ChargeStatus::Unreported
        ) {
            PowerSource::Battery
        } else {
            PowerSource::Mains
        }
    }

    fn breakdown_on_battery(&self) -> Vec<Section> {
        vec![Section::new("Battery feeds", self.battery_feeds_table())]
    }

    fn battery_feeds_table(&self) -> SectionTable {
        dictionary_table([LabeledValue::new("Laptop", Quantity::power(self.power))])
    }

    fn breakdown_on_ac(&self) -> Vec<Section> {
        vec![Section::new("Charger feeds", self.charger_feeds_table())]
    }

    fn charger_feeds_table(&self) -> SectionTable {
        let laptop = self
            .expected_laptop
            .map(|laptop| LabeledValue::new("Laptop (estimated)", Quantity::power(laptop)));
        dictionary_table(
            laptop
                .into_iter()
                .chain([LabeledValue::new("Battery", Quantity::power(self.power))]),
        )
    }
}

fn charger_limit_table(charger: &ChargerSupply) -> SectionTable {
    dictionary_table(charger.limit.iter().flat_map(|limit| {
        [
            LabeledValue::new("Limit", Quantity::power(limit.power())),
            LabeledValue::new("Voltage", Quantity::voltage(limit.voltage)),
            LabeledValue::new("Current", Quantity::current(limit.current)),
        ]
    }))
}

impl ReadingSections for ChargerSupply {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::new("Limits", charger_limit_table(self))]
    }
}

impl PowerDraw {
    pub fn reading_row(self) -> ReadingRow<'static> {
        let glyph = if self.power_source() == PowerSource::Battery {
            Phosphor::Plugs(Weight::Regular).into()
        } else {
            Phosphor::PlugsConnected(Weight::Regular).into()
        };
        let state = if self.status != ChargeStatus::Discharging {
            ReadingDisplayState::Idle
        } else if self.power > Power::new::<watt>(POWER_CRITICAL_W) {
            ReadingDisplayState::Critical
        } else if self.power > Power::new::<watt>(POWER_WARN_W) {
            ReadingDisplayState::Warning
        } else {
            ReadingDisplayState::Idle
        };
        let fill = self.power / Power::new::<watt>(POWER_GAUGE_MAX_W);
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Unlabeled,
                gauge: ReadoutGauge::Gauged { fill, live: fill },
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                "Power draw",
                glyph,
                state,
                Box::new(move |clock| self.hover_card(clock)),
            )
        }
    }

    pub fn hover_card(&self, _clock: Clock) -> Vec<Section> {
        if self.power_source() == PowerSource::Battery {
            self.breakdown_on_battery()
        } else {
            self.breakdown_on_ac()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerSource {
    Battery,
    Mains,
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::{
        SnapshotSource, battery_power_profiles::upower_battery_state::ChargerLimit,
    };
    use uom::si::{
        electric_current::ampere,
        electric_potential::volt,
        f64::{ElectricCurrent, ElectricPotential},
    };

    use super::*;

    fn draw(status: ChargeStatus) -> PowerDraw {
        PowerDraw {
            power: Power::new::<watt>(19.7),
            status,
            expected_laptop: Some(Power::new::<watt>(8.9)),
        }
    }

    #[test]
    fn on_ac_shows_the_charger_and_what_it_feeds() {
        let charger = ChargerSupply {
            limit: Some(ChargerLimit {
                voltage: ElectricPotential::new::<volt>(20.0),
                current: ElectricCurrent::new::<ampere>(3.25),
            }),
        };
        assert_eq!(
            charger_limit_table(&charger),
            dictionary_table([
                LabeledValue::new("Limit", Quantity::power(Power::new::<watt>(65.0))),
                LabeledValue::new(
                    "Voltage",
                    Quantity::voltage(ElectricPotential::new::<volt>(20.0))
                ),
                LabeledValue::new(
                    "Current",
                    Quantity::current(ElectricCurrent::new::<ampere>(3.25))
                ),
            ])
        );
        assert_eq!(ChargerSupply::NAME, "Charger");
        assert_eq!(
            draw(ChargeStatus::Charging).charger_feeds_table(),
            dictionary_table([
                LabeledValue::new(
                    "Laptop (estimated)",
                    Quantity::power(Power::new::<watt>(8.9))
                ),
                LabeledValue::new("Battery", Quantity::power(Power::new::<watt>(19.7))),
            ])
        );
    }

    #[test]
    fn on_battery_the_battery_feeds_the_laptop() {
        assert_eq!(
            draw(ChargeStatus::Discharging).battery_feeds_table(),
            dictionary_table([LabeledValue::new(
                "Laptop",
                Quantity::power(Power::new::<watt>(19.7))
            )])
        );
    }
}
