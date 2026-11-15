use iced::futures::TryFutureExt;
use sottomano_desktop_sinks::screen_brightness_color::umbra_mode_writes;
use sottomano_desktop_sources::{
    SnapshotSource,
    screen_brightness_color::{solar_switch_schedule::ColorMode, umbra_dark_mode::DarkMode},
    system_protocol_clients::shell_command_output::WatchedCommandError,
};

use crate::{
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        toggle_control::{SwitchPosition, ToggleableSource},
    },
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
};

pub fn dark_mode_reading_row(mode: &DarkMode) -> ReadingRow<'_> {
    let glyph: IconGlyph = match mode.color_mode {
        ColorMode::Dark => Phosphor::Moon(Weight::Regular).into(),
        ColorMode::Light => Phosphor::Sun(Weight::Regular).into(),
    };
    ReadingRow::glyph_only(
        DarkMode::NAME,
        glyph,
        ReadingDisplayState::Idle,
        Box::new(|_| Vec::new()),
    )
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
