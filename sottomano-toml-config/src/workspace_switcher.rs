use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorkspaceSwitcherConfig {
    pub preview_scale: f32,
}

impl Default for WorkspaceSwitcherConfig {
    fn default() -> Self {
        Self {
            preview_scale: 0.19,
        }
    }
}
