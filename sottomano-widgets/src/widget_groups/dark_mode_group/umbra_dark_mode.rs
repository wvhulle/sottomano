use sottomano_desktop_sinks::screen_brightness_color::umbra_mode_writes;
use sottomano_desktop_sources::{
    screen_brightness_color::umbra_dark_mode::DarkMode,
    system_protocol_clients::shell_command_output,
};

use crate::{
    clock::Clock,
    group_items::{group_item_reading::GroupItemReading, toggle_control::ToggleableSource},
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
        if self.dark {
            Phosphor::Moon(Weight::Regular).into()
        } else {
            Phosphor::Sun(Weight::Regular).into()
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([])
    }
}

impl ToggleableSource for DarkMode {
    fn toggled_value(&self) -> Self {
        DarkMode { dark: !self.dark }
    }

    fn apply_toggle(&self) -> Result<(), shell_command_output::Error> {
        umbra_mode_writes::toggle_mode()
    }

    fn switched_on(&self) -> bool {
        self.dark
    }

    fn press_action_text(&self) -> &'static str {
        if self.dark {
            "switch apps and the desktop to the light theme"
        } else {
            "switch apps and the desktop to the dark theme"
        }
    }
}
