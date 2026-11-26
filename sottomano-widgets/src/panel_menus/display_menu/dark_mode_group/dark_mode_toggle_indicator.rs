use iced::futures::TryFutureExt;
use sottomano_desktop_sinks::screen_brightness_color::umbra_mode_writes;
use sottomano_desktop_sources::{
    SnapshotSource,
    screen_brightness_color::{solar_switch_schedule::ColorMode, umbra_dark_mode::DarkMode},
    system_protocol_clients::shell_command_output::WatchedCommandError,
};

use super::{DarkModeGroup, DarkModeInteraction, DarkModeSource};
use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_state::IndicatorState,
        toggle_control::{SwitchPosition, ToggleableSource},
    },
};

pub fn dark_mode_indicator(mode: &DarkMode) -> GroupIndicator<'_> {
    let glyph: IconGlyph = match mode.color_mode {
        ColorMode::Dark => Phosphor::Moon(Weight::Regular).into(),
        ColorMode::Light => Phosphor::Sun(Weight::Regular).into(),
    };
    GroupIndicator::glyph_only(DarkMode::NAME, glyph, IndicatorState::Idle)
        .with_hover_card(Box::new(|_| Vec::new()))
}

impl ToggleableSource for DarkMode {
    fn toggled_value(&self) -> Self {
        DarkMode {
            color_mode: self.color_mode.opposite_mode(),
        }
    }

    fn apply_toggle(
        &self,
    ) -> impl Future<Output = Result<(), WatchedCommandError>> + Send + 'static {
        umbra_mode_writes::toggle_mode().map_err(WatchedCommandError::from)
    }

    fn switch_position(&self) -> SwitchPosition {
        match self.color_mode {
            ColorMode::Dark => SwitchPosition::On,
            ColorMode::Light => SwitchPosition::Off,
        }
    }

    fn press_action_text(&self) -> &'static str {
        match self.color_mode {
            ColorMode::Dark => "switch apps and the desktop to the light theme",
            ColorMode::Light => "switch apps and the desktop to the dark theme",
        }
    }
}

impl DarkModeGroup {
    pub(super) fn mode_toggle_rows<'view>(
        &'view self,
        mode: &'view DarkMode,
    ) -> Vec<GroupIndicator<'view>> {
        self.wired_mode_toggle_indicator(dark_mode_indicator(mode))
            .on_switch_press(
                mode.switch_position(),
                mode.press_action_text(),
                DarkModeInteraction::ModeTogglePressed,
            )
            .when_shown()
    }

    fn wired_mode_toggle_indicator<'view>(
        &'view self,
        mode_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        mode_indicator
            .with_hover_card(Box::new(move |_| self.mode_toggle.hover_card()))
            .refreshed_by(DarkModeInteraction::Refresh(DarkModeSource::Mode))
    }
}
