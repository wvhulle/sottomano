use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct MediaLyricsConfig {
    pub lrclib_lookup: bool,
}

impl Default for MediaLyricsConfig {
    fn default() -> Self {
        Self {
            lrclib_lookup: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lrclib_lookup_is_on_until_disabled() {
        assert!(MediaLyricsConfig::default().lrclib_lookup);
        let config: MediaLyricsConfig = toml::from_str("lrclib_lookup = false\n").unwrap();
        assert!(!config.lrclib_lookup);
    }

    #[test]
    fn unknown_fields_are_rejected() {
        assert!(toml::from_str::<MediaLyricsConfig>("provider = \"genius\"\n").is_err());
    }
}
