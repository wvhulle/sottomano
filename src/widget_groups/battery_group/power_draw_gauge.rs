use crate::{
    data::source::sysfs_sensors::battery::{Battery, ChargeStatus, ChargerLimit},
    widgets::{
        bar_widget::{BarWidget, Glyph, Level, Unit, Value},
        font_awesome::Solid,
        tooltip_body::{Section, TooltipBody, row, section},
    },
};

const POWER_WARN_W: f64 = 10.0;
const POWER_CRITICAL_W: f64 = 20.0;
const POWER_GAUGE_MAX_W: f64 = 40.0;

pub struct PowerDraw {
    watts: f64,
    status: ChargeStatus,
    charger: Option<ChargerLimit>,
    expected_laptop_w: Option<f64>,
}

impl PowerDraw {
    pub fn of(battery: &Battery, expected_laptop_w: Option<f64>) -> Self {
        PowerDraw {
            watts: battery.power_w,
            status: battery.status,
            charger: battery.charger,
            expected_laptop_w,
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
            vec![row("Laptop", watts_text(self.watts))],
        )]
    }

    fn breakdown_on_ac(&self) -> Vec<Section> {
        let laptop = self.expected_laptop_w.map_or_else(
            || row("Laptop", "not measured"),
            |laptop_w| row("Laptop (estimated)", watts_text(laptop_w)),
        );
        let charger = self.charger.map(|limit| {
            section(
                "Charger",
                vec![
                    row("Limit", watts_text(limit.watts())),
                    row("Voltage", format!("{:.0} V", limit.volts)),
                    row("Current", format!("{:.2} A", limit.amps)),
                ],
            )
        });
        let feeds = section(
            "Charger feeds",
            vec![laptop, row("Battery", watts_text(self.watts))],
        );
        charger.into_iter().chain([feeds]).collect()
    }
}

fn watts_text(watts: f64) -> String {
    format!("{watts:.1} W")
}

impl BarWidget for PowerDraw {
    fn icon(&self) -> Glyph {
        if self.on_battery() {
            Solid::PlugCircleMinus.into()
        } else {
            Solid::PlugCirclePlus.into()
        }
    }

    fn level(&self) -> Level {
        if self.status != ChargeStatus::Discharging {
            Level::Ok
        } else if self.watts > POWER_CRITICAL_W {
            Level::Critical
        } else if self.watts > POWER_WARN_W {
            Level::Warning
        } else {
            Level::Ok
        }
    }

    fn value(&self) -> Option<Value> {
        let fraction = (self.watts / POWER_GAUGE_MAX_W * 100.0).clamp(0.0, 100.0);
        Some(Value::Gauge {
            percent: fraction.round() as i64,
            unit: Unit::Watt,
        })
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
    use super::*;

    fn draw(status: ChargeStatus) -> PowerDraw {
        PowerDraw {
            watts: 19.7,
            status,
            charger: Some(ChargerLimit {
                volts: 20.0,
                amps: 3.25,
            }),
            expected_laptop_w: Some(8.9),
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
