pub mod gamepad_bindings;

use serde::{Deserialize, Serialize};

use self::gamepad_bindings::GamepadBindingsConfig;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct GamingMenuConfig {
    pub gamepad: GamepadBindingsConfig,
}
