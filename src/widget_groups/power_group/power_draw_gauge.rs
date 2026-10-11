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
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

const POWER_WARN_W: f64 = 10.0;
const POWER_CRITICAL_W: f64 = 20.0;
const POWER_GAUGE_MAX_W: f64 = 40.0;

pub struct PowerDraw {
    power: Power,
    status: Option<ChargeStatus>,
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
        matches!(self.status, Some(ChargeStatus::Discharging) | None)
    }

    fn breakdown_on_battery(&self) -> TooltipBody {
        TooltipBody::sections(&[Section::dictionary(
            "Battery feeds",
            [KeyValue::new("Laptop", power_text(self.power))],
        )])
    }

    fn breakdown_on_ac(&self) -> TooltipBody {
        let laptop = self
            .expected_laptop
            .map(|laptop| KeyValue::new("Laptop (estimated)", power_text(laptop)));
        let feeds = Section::dictionary(
            "Charger feeds",
            laptop
                .into_iter()
                .chain([KeyValue::new("Battery", power_text(self.power))]),
        );
        let Some(limit) = self.charger else {
            return TooltipBody::sections(&[feeds]);
        };
        TooltipBody::sections(&[
            Section::dictionary(
                "Charger",
                [
                    KeyValue::new("Limit", power_text(limit.power())),
                    KeyValue::new(
                        "Voltage",
                        format!("{:.0}", limit.voltage.into_format_args(volt, Abbreviation)),
                    ),
                    KeyValue::new(
                        "Current",
                        format!(
                            "{:.2}",
                            limit.current.into_format_args(ampere, Abbreviation)
                        ),
                    ),
                ],
            ),
            feeds,
        ])
    }
}

fn power_text(power: Power) -> String {
    format!("{:.1}", power.into_format_args(watt, Abbreviation))
}

impl BarWidget for PowerDraw {
    fn name(&self) -> &'static str {
        "Power draw"
    }

    fn icon(&self) -> Glyph {
        if self.on_battery() {
            Phosphor::Plugs(self.state().weight()).into()
        } else {
            Phosphor::PlugsConnected(self.state().weight()).into()
        }
    }

    fn state(&self) -> WidgetState {
        if self.status != Some(ChargeStatus::Discharging) {
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
        if self.on_battery() {
            self.breakdown_on_battery()
        } else {
            self.breakdown_on_ac()
        }
    }
}

#[cfg(test)]
mod tests {
    use uom::si::f64::{ElectricCurrent, ElectricPotential};

    use super::*;

    fn draw(status: ChargeStatus) -> PowerDraw {
        PowerDraw {
            power: Power::new::<watt>(19.7),
            status: Some(status),
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
            TooltipBody::sections(&[
                Section::dictionary(
                    "Charger",
                    [
                        KeyValue::new("Limit", "65.0 W"),
                        KeyValue::new("Voltage", "20 V"),
                        KeyValue::new("Current", "3.25 A"),
                    ],
                ),
                Section::dictionary(
                    "Charger feeds",
                    [
                        KeyValue::new("Laptop (estimated)", "8.9 W"),
                        KeyValue::new("Battery", "19.7 W"),
                    ],
                ),
            ])
        );
    }

    #[test]
    fn on_battery_the_battery_feeds_the_laptop() {
        assert_eq!(
            draw(ChargeStatus::Discharging).breakdown_on_battery(),
            TooltipBody::sections(&[Section::dictionary(
                "Battery feeds",
                [KeyValue::new("Laptop", "19.7 W")],
            )])
        );
    }
}
