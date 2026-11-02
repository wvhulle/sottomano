use sottomano_sources::battery_power_profiles::upower_battery_state::{
    Battery, ChargeStatus, ChargerSupply,
};
use uom::si::{
    f64::{Power, Ratio},
    power::watt,
};

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading, reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, ReadingSections, Section},
    widgets::{
        icon_glyph::IconGlyph,
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
    status: Option<ChargeStatus>,
    expected_laptop: Option<Power>,
}

impl PowerDraw {
    pub fn of(battery: &Battery, expected_laptop: Option<Power>) -> Self {
        PowerDraw {
            power: battery.power,
            status: battery.status,
            expected_laptop,
        }
    }

    pub fn on_battery(&self) -> bool {
        matches!(self.status, Some(ChargeStatus::Discharging) | None)
    }

    fn breakdown_on_battery(&self) -> HoverCardBody {
        HoverCardBody::sections([Section::dictionary([LabeledValue::new(
            "Laptop",
            Quantity::power(self.power),
        )])
        .titled("Battery feeds")])
    }

    fn breakdown_on_ac(&self) -> HoverCardBody {
        let laptop = self
            .expected_laptop
            .map(|laptop| LabeledValue::new("Laptop (estimated)", Quantity::power(laptop)));
        HoverCardBody::sections([Section::dictionary(
            laptop
                .into_iter()
                .chain([LabeledValue::new("Battery", Quantity::power(self.power))]),
        )
        .titled("Charger feeds")])
    }
}

impl ReadingSections for ChargerSupply {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::dictionary(self.limit.iter().flat_map(|limit| {
            [
                LabeledValue::new("Limit", Quantity::power(limit.power())),
                LabeledValue::new("Voltage", Quantity::voltage(limit.voltage)),
                LabeledValue::new("Current", Quantity::current(limit.current)),
            ]
        }))]
    }
}

impl GroupItemReading for PowerDraw {
    fn name(&self) -> &'static str {
        "Power draw"
    }

    fn icon(&self) -> IconGlyph {
        if self.on_battery() {
            Phosphor::Plugs(Weight::Regular).into()
        } else {
            Phosphor::PlugsConnected(Weight::Regular).into()
        }
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        if self.status != Some(ChargeStatus::Discharging) {
            ReadingDisplayState::Idle
        } else if self.power > Power::new::<watt>(POWER_CRITICAL_W) {
            ReadingDisplayState::Critical
        } else if self.power > Power::new::<watt>(POWER_WARN_W) {
            ReadingDisplayState::Warning
        } else {
            ReadingDisplayState::Idle
        }
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(self.power / Power::new::<watt>(POWER_GAUGE_MAX_W))
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        if self.on_battery() {
            self.breakdown_on_battery()
        } else {
            self.breakdown_on_ac()
        }
    }
}

#[cfg(test)]
mod tests {
    use sottomano_sources::{
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
            status: Some(status),
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
            charger.sections(Clock::real()),
            [Section::dictionary([
                LabeledValue::new("Limit", Quantity::power(Power::new::<watt>(65.0))),
                LabeledValue::new(
                    "Voltage",
                    Quantity::voltage(ElectricPotential::new::<volt>(20.0))
                ),
                LabeledValue::new(
                    "Current",
                    Quantity::current(ElectricCurrent::new::<ampere>(3.25))
                ),
            ])]
        );
        assert_eq!(ChargerSupply::NAME, "Charger");
        assert_eq!(
            draw(ChargeStatus::Charging).breakdown_on_ac(),
            HoverCardBody::sections([Section::dictionary([
                LabeledValue::new(
                    "Laptop (estimated)",
                    Quantity::power(Power::new::<watt>(8.9))
                ),
                LabeledValue::new("Battery", Quantity::power(Power::new::<watt>(19.7))),
            ])
            .titled("Charger feeds")])
        );
    }

    #[test]
    fn on_battery_the_battery_feeds_the_laptop() {
        assert_eq!(
            draw(ChargeStatus::Discharging).breakdown_on_battery(),
            HoverCardBody::sections([Section::dictionary([LabeledValue::new(
                "Laptop",
                Quantity::power(Power::new::<watt>(19.7))
            )])
            .titled("Battery feeds"),])
        );
    }
}
