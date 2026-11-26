use sottomano_desktop_sources::battery_power_profiles::upower_battery_state::{
    Battery, ChargeStatus,
};
use uom::si::{f64::Ratio, ratio::percent};

use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::indicator_state::IndicatorState,
};

const FULL_LEVEL_PERCENT: f64 = 80.0;
const HIGH_LEVEL_PERCENT: f64 = 55.0;
const MEDIUM_LEVEL_PERCENT: f64 = 30.0;
const LOW_LEVEL_PERCENT: f64 = 15.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargerConnection {
    Charging,
    PluggedIdle,
    Unplugged,
    Unreported,
}

impl From<ChargeStatus> for ChargerConnection {
    fn from(status: ChargeStatus) -> Self {
        match status {
            ChargeStatus::Charging => ChargerConnection::Charging,
            ChargeStatus::Full | ChargeStatus::NotCharging => ChargerConnection::PluggedIdle,
            ChargeStatus::Discharging => ChargerConnection::Unplugged,
            ChargeStatus::Unreported => ChargerConnection::Unreported,
        }
    }
}

impl From<&Battery> for ChargerConnection {
    fn from(battery: &Battery) -> Self {
        ChargerConnection::from(battery.status)
    }
}

impl ChargerConnection {
    #[must_use]
    pub fn feeds_from_charger(self) -> bool {
        matches!(
            self,
            ChargerConnection::Charging | ChargerConnection::PluggedIdle
        )
    }

    #[must_use]
    pub fn battery_glyph(self, capacity: Ratio) -> IconGlyph {
        match self {
            ChargerConnection::Charging => Phosphor::BatteryCharging(Weight::Regular).into(),
            ChargerConnection::PluggedIdle => Phosphor::PlugCharging(Weight::Regular).into(),
            ChargerConnection::Unplugged | ChargerConnection::Unreported => {
                battery_level_glyph(capacity)
            }
        }
    }

    #[must_use]
    pub fn indicator_state(self, unplugged_state: IndicatorState) -> IndicatorState {
        if self.feeds_from_charger() {
            IndicatorState::Live
        } else {
            unplugged_state
        }
    }
}

fn battery_level_glyph(capacity: Ratio) -> IconGlyph {
    let level_percent = capacity.get::<percent>();
    let level_glyph = if level_percent > FULL_LEVEL_PERCENT {
        Phosphor::BatteryFull(Weight::Regular)
    } else if level_percent > HIGH_LEVEL_PERCENT {
        Phosphor::BatteryHigh(Weight::Regular)
    } else if level_percent > MEDIUM_LEVEL_PERCENT {
        Phosphor::BatteryMedium(Weight::Regular)
    } else if level_percent > LOW_LEVEL_PERCENT {
        Phosphor::BatteryLow(Weight::Regular)
    } else {
        Phosphor::BatteryWarning(Weight::Regular)
    };
    level_glyph.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charge_status_maps_to_charger_connection() {
        assert_eq!(
            ChargerConnection::from(ChargeStatus::Charging),
            ChargerConnection::Charging
        );
        assert_eq!(
            ChargerConnection::from(ChargeStatus::Full),
            ChargerConnection::PluggedIdle
        );
        assert_eq!(
            ChargerConnection::from(ChargeStatus::NotCharging),
            ChargerConnection::PluggedIdle
        );
        assert_eq!(
            ChargerConnection::from(ChargeStatus::Discharging),
            ChargerConnection::Unplugged
        );
        assert_eq!(
            ChargerConnection::from(ChargeStatus::Unreported),
            ChargerConnection::Unreported
        );
    }

    #[test]
    fn plugged_in_battery_is_tinted_live() {
        assert_eq!(
            ChargerConnection::Charging.indicator_state(IndicatorState::Critical),
            IndicatorState::Live
        );
        assert_eq!(
            ChargerConnection::PluggedIdle.indicator_state(IndicatorState::Idle),
            IndicatorState::Live
        );
        assert_eq!(
            ChargerConnection::Unplugged.indicator_state(IndicatorState::Warning),
            IndicatorState::Warning
        );
    }

    #[test]
    fn unplugged_glyph_follows_battery_level() {
        let unplugged_glyph = |level_percent| {
            ChargerConnection::Unplugged.battery_glyph(Ratio::new::<percent>(level_percent))
        };
        assert_eq!(
            unplugged_glyph(95.0),
            Phosphor::BatteryFull(Weight::Regular).into()
        );
        assert_eq!(
            unplugged_glyph(60.0),
            Phosphor::BatteryHigh(Weight::Regular).into()
        );
        assert_eq!(
            unplugged_glyph(40.0),
            Phosphor::BatteryMedium(Weight::Regular).into()
        );
        assert_eq!(
            unplugged_glyph(20.0),
            Phosphor::BatteryLow(Weight::Regular).into()
        );
        assert_eq!(
            unplugged_glyph(10.0),
            Phosphor::BatteryWarning(Weight::Regular).into()
        );
        assert_eq!(
            ChargerConnection::Charging.battery_glyph(Ratio::new::<percent>(10.0)),
            Phosphor::BatteryCharging(Weight::Regular).into()
        );
    }
}
