use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct LaunchedPrograms {
    pub terminal_command_prefix: Vec<String>,
    pub color_temperature_service: String,
}

impl Default for LaunchedPrograms {
    fn default() -> Self {
        Self {
            terminal_command_prefix: ["kitty", "-e"].map(String::from).to_vec(),
            color_temperature_service: String::from("screen-temperature.service"),
        }
    }
}
