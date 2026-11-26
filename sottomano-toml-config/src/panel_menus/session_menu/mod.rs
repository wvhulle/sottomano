pub mod tray;

use serde::{Deserialize, Serialize};

use self::tray::TrayConfig;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct SessionMenuConfig {
    pub tray: TrayConfig,
}
