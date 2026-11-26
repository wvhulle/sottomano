pub mod caldav_account;
pub mod rain_alerts;
pub mod session_focus;

use serde::{Deserialize, Serialize};

use self::{
    caldav_account::CaldavAccountConfig, rain_alerts::RainAlertsConfig,
    session_focus::SessionFocusConfig,
};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct AgendaMenuConfig {
    pub caldav: CaldavAccountConfig,
    pub weather: WeatherGroupConfig,
    pub session_focus: SessionFocusConfig,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct WeatherGroupConfig {
    pub rain_alerts: RainAlertsConfig,
}
