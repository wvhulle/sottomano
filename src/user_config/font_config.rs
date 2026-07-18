//! The `font` config group (matches odin's `FontConfig.qml`).

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Font {
    pub family: String,
    pub mono: String,
    pub size: u16,
}

impl Default for Font {
    fn default() -> Self {
        Self {
            family: "sans-serif".to_string(),
            mono: "monospace".to_string(),
            size: 12,
        }
    }
}
