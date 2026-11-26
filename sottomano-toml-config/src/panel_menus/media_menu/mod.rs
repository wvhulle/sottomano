pub mod media_lyrics;

use serde::{Deserialize, Serialize};

use self::media_lyrics::MediaLyricsConfig;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct MediaMenuConfig {
    pub media: MediaGroupConfig,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct MediaGroupConfig {
    pub lyrics: MediaLyricsConfig,
}
