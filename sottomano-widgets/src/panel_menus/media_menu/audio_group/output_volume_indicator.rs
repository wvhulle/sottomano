use iced::Function;
use sottomano_desktop_sources::media_playback_audio::pipewire_audio::AudioState;
use sottomano_toml_config::Config;

use super::{AudioGroup, AudioMessage, ControlKind, resolved_levels};
use crate::group_indicators::group_indicator::GroupIndicator;

impl AudioGroup {
    pub(super) fn output_volume_rows<'view>(
        &'view self,
        config: &'view Config,
        state: &'view AudioState,
    ) -> Option<GroupIndicator<'view>> {
        resolved_levels(&state.sink).map(|levels| {
            GroupIndicator::slider(
                &self.sink_control,
                config,
                AudioMessage::Control.with(ControlKind::Sink),
            )
            .with_hover_card(Box::new(move |_| {
                self.channel_level_sections(ControlKind::Sink, levels, &self.sink_control)
            }))
        })
    }
}
