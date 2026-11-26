use iced::Function;
use sottomano_desktop_sources::media_playback_audio::pipewire_audio::AudioState;
use sottomano_toml_config::Config;

use super::{AudioGroup, AudioMessage, ControlKind, resolved_levels};
use crate::group_indicators::group_indicator::GroupIndicator;

impl AudioGroup {
    pub(super) fn input_volume_rows<'view>(
        &'view self,
        config: &'view Config,
        state: &'view AudioState,
    ) -> Option<GroupIndicator<'view>> {
        resolved_levels(&state.source).map(|levels| {
            GroupIndicator::slider(
                &self.source_control,
                config,
                AudioMessage::Control.with(ControlKind::Source),
            )
            .with_hover_card(Box::new(move |_| {
                self.channel_level_sections(ControlKind::Source, levels, &self.source_control)
            }))
        })
    }
}
