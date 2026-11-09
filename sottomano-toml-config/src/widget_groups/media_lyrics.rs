use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LrclibLookup {
    #[default]
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct MediaLyricsConfig {
    pub lrclib_lookup: LrclibLookup,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lrclib_lookup_can_be_disabled() {
        let config: MediaLyricsConfig = toml::from_str("lrclib_lookup = \"disabled\"\n").unwrap();
        assert_eq!(config.lrclib_lookup, LrclibLookup::Disabled);
    }

    #[test]
    fn unknown_fields_are_rejected() {
        assert!(toml::from_str::<MediaLyricsConfig>("provider = \"genius\"\n").is_err());
    }
}
