use sottomano_desktop_sinks::screen_brightness_color::umbra_mode_writes;
use sottomano_desktop_sources::{
    screen_brightness_color::{solar_switch_schedule::ColorMode, umbra_dark_mode::DarkMode},
    system_protocol_clients::shell_command_output,
};

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading,
        toggle_control::{SwitchPosition, ToggleableSource},
    },
    hover_cards::hover_card_body::HoverCardBody,
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
};

impl GroupItemReading for DarkMode {
    fn name(&self) -> &'static str {
        "Dark mode"
    }

    fn icon(&self) -> IconGlyph {
        match self.color_mode {
            ColorMode::Dark => Phosphor::Moon(Weight::Regular).into(),
            ColorMode::Light => Phosphor::Sun(Weight::Regular).into(),
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([])
    }
}

impl ToggleableSource for DarkMode {
    fn toggled_value(&self) -> Self {
        DarkMode {
            color_mode: self.color_mode.opposite_mode(),
        }
    }

    fn apply_toggle(
        &self,
    ) -> impl Future<Output = Result<(), shell_command_output::Error>> + Send + 'static {
        umbra_mode_writes::toggle_mode()
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
