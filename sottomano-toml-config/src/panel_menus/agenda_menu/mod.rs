pub mod caldav_account;
pub mod rain_alerts;
pub mod session_focus;

use serde::{Deserialize, Serialize};

use self::{
    caldav_account::CaldavAccountConfig, rain_alerts::RainAlertsConfig,
    session_focus::SessionFocusConfig,
};
use crate::panel_pin_setting::PanelPinSetting;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct AgendaMenuConfig {
    pub clock: ClockGroupConfig,
    pub caldav: CaldavAccountConfig,
    pub weather: WeatherGroupConfig,
    pub session_focus: SessionFocusConfig,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct WeatherGroupConfig {
    pub rain_alerts: RainAlertsConfig,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ClockGroupConfig {
    pub local_time: LocalTimeConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct LocalTimeConfig {
    pub pin: PanelPinSetting,
}

impl Default for LocalTimeConfig {
    fn default() -> Self {
        Self {
            pin: PanelPinSetting::Forced,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AgendaMenuConfig;
    use crate::panel_pin_setting::PanelPinSetting;

    #[test]
    fn the_local_time_is_forced_onto_the_panel_by_default() {
        assert_eq!(
            AgendaMenuConfig::default().clock.local_time.pin,
            PanelPinSetting::Forced
        );
    }

    #[test]
    fn the_local_time_pin_can_be_freed() {
        let config: AgendaMenuConfig =
            toml::from_str("[clock.local_time]\npin = \"free\"\n").unwrap();
        assert_eq!(config.clock.local_time.pin, PanelPinSetting::Free);
    }
}
