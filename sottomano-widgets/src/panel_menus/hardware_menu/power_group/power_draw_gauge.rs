use chrono::Locale;
use sottomano_desktop_sources::battery_power_profiles::upower_battery_state::{
    Battery, ChargeStatus, ChargerSupply,
};
use uom::si::{f64::Power, power::watt};

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionTable, dictionary_table},
    panel_menus::hardware_menu::power_group::charger_connection::ChargerConnection,
    widgets::{localized_quantities::Quantity, value_cells::LabeledValue},
};

const POWER_WARN_W: f64 = 10.0;
const POWER_CRITICAL_W: f64 = 20.0;

pub const POWER_FLOW_INDICATOR_NAME: &str = "Power flow";

#[derive(Debug, Clone, Copy)]
pub enum LaptopDrawEstimate {
    NotModelled,
    Modelled(Power),
}

pub struct PowerDraw {
    power: Power,
    status: ChargeStatus,
    expected_laptop: LaptopDrawEstimate,
}

impl PowerDraw {
    pub fn battery_draw(battery: &Battery, expected_laptop: LaptopDrawEstimate) -> Self {
        PowerDraw {
            power: battery.power,
            status: battery.status,
            expected_laptop,
        }
    }

    pub fn connection(&self) -> ChargerConnection {
        ChargerConnection::from(self.status)
    }

    fn breakdown_on_battery(&self) -> Vec<Section> {
        vec![Section::new("On battery", self.battery_feeds_table())]
    }

    fn battery_feeds_table(&self) -> SectionTable {
        dictionary_table([LabeledValue::new("Laptop", Quantity::power(self.power))])
    }

    fn breakdown_on_ac(&self) -> Vec<Section> {
        vec![Section::new("Power split", self.charger_feeds_table())]
    }

    fn charger_feeds_table(&self) -> SectionTable {
        let into_battery = LabeledValue::new("Into battery", Quantity::power(self.power));
        match self.expected_laptop {
            LaptopDrawEstimate::Modelled(laptop) => dictionary_table([
                LabeledValue::new("Laptop (estimated)", Quantity::power(laptop)),
                into_battery,
            ]),
            LaptopDrawEstimate::NotModelled => dictionary_table([into_battery]),
        }
    }

    fn flow_label(&self, locale: Locale) -> String {
        match self.connection() {
            ChargerConnection::Charging => Quantity::signed_power(self.power),
            ChargerConnection::PluggedIdle
            | ChargerConnection::Unplugged
            | ChargerConnection::Unreported => Quantity::power(self.power),
        }
        .localized_text(locale)
    }

    fn unplugged_state(&self) -> IndicatorState {
        if self.status != ChargeStatus::Discharging {
            IndicatorState::Idle
        } else if self.power > Power::new::<watt>(POWER_CRITICAL_W) {
            IndicatorState::Critical
        } else if self.power > Power::new::<watt>(POWER_WARN_W) {
            IndicatorState::Warning
        } else {
            IndicatorState::Idle
        }
    }
}

fn charger_rating_table(charger: &ChargerSupply) -> SectionTable {
    match charger {
        ChargerSupply::Online(limit) => dictionary_table([
            LabeledValue::new("Rated", Quantity::power(limit.power())),
            LabeledValue::new("Voltage", Quantity::voltage(limit.voltage)),
            LabeledValue::new("Current", Quantity::current(limit.current)),
        ]),
        ChargerSupply::NoChargerOnline => dictionary_table([]),
    }
}

pub fn charger_supply_sections(charger_supply: &ChargerSupply) -> Vec<Section> {
    match charger_supply {
        ChargerSupply::Online(_) => vec![Section::new(
            "Charger connected",
            charger_rating_table(charger_supply),
        )],
        ChargerSupply::NoChargerOnline => Vec::new(),
    }
}

impl PowerDraw {
    pub fn indicator<'indicator>(self, locale: Locale) -> GroupIndicator<'indicator> {
        let connection = self.connection();
        let glyph = if connection.feeds_from_charger() {
            Phosphor::PlugCharging(Weight::Regular).into()
        } else {
            Phosphor::Lightning(Weight::Regular).into()
        };
        let state = connection.indicator_state(self.unplugged_state());
        GroupIndicator::glyph_only(POWER_FLOW_INDICATOR_NAME, glyph, state)
            .with_readout(IndicatorReadout {
                label: ReadoutLabel::Labeled(self.flow_label(locale)),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            })
            .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        if self.connection().feeds_from_charger() {
            self.breakdown_on_ac()
        } else {
            self.breakdown_on_battery()
        }
    }
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
            expected_laptop: LaptopDrawEstimate::Modelled(Power::new::<watt>(8.9)),
        }
    }

    #[test]
    fn on_ac_shows_the_charger_and_what_it_feeds() {
        let charger = ChargerSupply::Online(ChargerLimit {
            voltage: ElectricPotential::new::<volt>(20.0),
            current: ElectricCurrent::new::<ampere>(3.25),
        });
        assert_eq!(
            charger_rating_table(&charger),
            dictionary_table([
                LabeledValue::new("Rated", Quantity::power(Power::new::<watt>(65.0))),
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
                LabeledValue::new("Into battery", Quantity::power(Power::new::<watt>(19.7))),
            ])
        );
    }

    #[test]
    fn flow_label_signs_power_going_into_the_battery() {
        assert_eq!(
            draw(ChargeStatus::Charging).flow_label(Locale::en_US),
            "+19.7 W"
        );
        assert_eq!(
            draw(ChargeStatus::Discharging).flow_label(Locale::en_US),
            "19.7 W"
        );
    }

    #[test]
    fn missing_charger_adds_no_section() {
        assert!(charger_supply_sections(&ChargerSupply::NoChargerOnline).is_empty());
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
