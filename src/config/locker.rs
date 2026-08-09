use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Locker {
    pub command: Vec<String>,
    pub lock_before_sleep: bool,
}

impl Default for Locker {
    fn default() -> Self {
        Self {
            command: vec!["hyprlock".to_owned()],
            lock_before_sleep: true,
        }
    }
}
