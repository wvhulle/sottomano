use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{
        electric_current::ampere,
        electric_potential::volt,
        f64::{Power, Ratio},
        power::watt,
    },
};

use crate::{
    data::source::sysfs_sensors::battery::{Battery, ChargeStatus, ChargerLimit},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::Phosphor,
        tooltip_body::{Section, TooltipBody, row, section},
        widget_state::WidgetState,
    },
};

const POWER_WARN_W: f64 = 10.0;
const POWER_CRITICAL_W: f64 = 20.0;
const POWER_GAUGE_MAX_W: f64 = 40.0;

pub struct PowerDraw {
    power: Power,
    status: ChargeStatus,
    charger: Option<ChargerLimit>,
    expected_laptop: Option<Power>,
}

impl PowerDraw {
    pub fn of(battery: &Battery, expected_laptop: Option<Power>) -> Self {
        PowerDraw {
            power: battery.power,
            status: battery.status,
            charger: battery.charger,
            expected_laptop,
        }
    }

    fn on_battery(&self) -> bool {
        matches!(
            self.status,
            ChargeStatus::Discharging | ChargeStatus::Unknown
        )
    }

    fn breakdown_on_battery(&self) -> Vec<Section> {
        vec![section(
            "Battery feeds",
            vec![row("Laptop", power_text(self.power))],
        )]
    }

    fn breakdown_on_ac(&self) -> Vec<Section> {
        let laptop = self.expected_laptop.map_or_else(
            || row("Laptop", "not measured"),
            |laptop| row("Laptop (estimated)", power_text(laptop)),
        );
        let charger = self.charger.map(|limit| {
            section(
                "Charger",
                vec![
                    row("Limit", power_text(limit.power())),
                    row(
                        "Voltage",
                        format!("{:.0}", limit.voltage.into_format_args(volt, Abbreviation)),
                    ),
                    row(
                        "Current",
                        format!(
                            "{:.2}",
                            limit.current.into_format_args(ampere, Abbreviation)
                        ),
                    ),
                ],
            )
        });
        let feeds = section(
            "Charger feeds",
            vec![laptop, row("Battery", power_text(self.power))],
        );
        charger.into_iter().chain([feeds]).collect()
    }
}

fn power_text(power: Power) -> String {
    format!("{:.1}", power.into_format_args(watt, Abbreviation))
}

impl BarWidget for PowerDraw {
    fn icon(&self) -> Glyph {
        if self.on_battery() {
            Phosphor::Plugs(self.state().weight()).into()
        } else {
            Phosphor::PlugsConnected(self.state().weight()).into()
        }
    }

    fn state(&self) -> WidgetState {
        if self.status != ChargeStatus::Discharging {
            WidgetState::Idle
        } else if self.power > Power::new::<watt>(POWER_CRITICAL_W) {
            WidgetState::Critical
        } else if self.power > Power::new::<watt>(POWER_WARN_W) {
            WidgetState::Warning
        } else {
            WidgetState::Idle
        }
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(self.power / Power::new::<watt>(POWER_GAUGE_MAX_W))
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Sections(if self.on_battery() {
            self.breakdown_on_battery()
        } else {
            self.breakdown_on_ac()
        })
    }
}

#[cfg(test)]
mod tests {
    use uom::si::f64::{ElectricCurrent, ElectricPotential};

    use super::*;

    fn draw(status: ChargeStatus) -> PowerDraw {
        PowerDraw {
            power: Power::new::<watt>(19.7),
            status,
            charger: Some(ChargerLimit {
                voltage: ElectricPotential::new::<volt>(20.0),
                current: ElectricCurrent::new::<ampere>(3.25),
            }),
            expected_laptop: Some(Power::new::<watt>(8.9)),
        }
    }

    #[test]
    fn on_ac_shows_the_charger_then_what_it_feeds() {
        assert_eq!(
            draw(ChargeStatus::Charging).breakdown_on_ac(),
            vec![
                section(
                    "Charger",
                    vec![
                        row("Limit", "65.0 W"),
                        row("Voltage", "20 V"),
                        row("Current", "3.25 A"),
                    ],
                ),
                section(
                    "Charger feeds",
                    vec![row("Laptop (estimated)", "8.9 W"), row("Battery", "19.7 W"),],
                ),
            ]
        );
    }

    #[test]
    fn on_battery_the_battery_feeds_the_laptop() {
        assert_eq!(
            draw(ChargeStatus::Discharging).breakdown_on_battery(),
            vec![section("Battery feeds", vec![row("Laptop", "19.7 W")])]
        );
    }
}
