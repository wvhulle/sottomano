pub mod audio_group;
pub mod media_group;
pub mod voice_dictation_group;

use sottomano_desktop_sources::desktop_windows_apps::desktop_entries::DesktopEntries;
use sottomano_toml_config::Config;

use self::{
    audio_group::AudioGroup, media_group::MediaGroup, voice_dictation_group::VoiceDictationGroup,
};
use crate::{clock::Clock, group_indicators::indicator_group_content::IndicatorGroupContent};

#[derive(Default)]
pub struct MediaMenu {
    pub media_group: MediaGroup,
    pub audio_group: AudioGroup,
    pub voice_dictation_group: VoiceDictationGroup,
}

impl MediaMenu {
    pub fn collapse(&mut self) {
        self.media_group.collapse();
        self.audio_group.collapse();
        self.voice_dictation_group.collapse();
    }

    #[must_use]
    pub fn group_contents<'view>(
        &'view self,
        config: &'view Config,
        desktop_entries: &'view DesktopEntries,
        clock: Clock,
    ) -> Vec<IndicatorGroupContent<'view>> {
        self.media_group
            .group_contents(config, desktop_entries, clock)
            .chain([
                self.audio_group.group_content(config),
                self.voice_dictation_group.group_content(),
            ])
            .collect()
    }
}
