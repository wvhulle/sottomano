use sottomano_sources::{
    screen_brightness_color::umbra_dark_mode::{self, DarkMode},
    system_protocol_clients::shell_command_output,
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        toggle_control::ToggleableSource,
        tooltip_body::TooltipBody,
    },
};

impl BarWidget for DarkMode {
    fn name(&self) -> &'static str {
        "Dark mode"
    }

    fn icon(&self) -> Glyph {
        if self.dark {
            Phosphor::Moon(Weight::Regular).into()
        } else {
            Phosphor::Sun(Weight::Regular).into()
        }
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([])
    }
}

impl ToggleableSource for DarkMode {
    fn toggled_value(&self) -> Self {
        DarkMode { dark: !self.dark }
    }

    fn apply_toggle(&self) -> Result<(), shell_command_output::Error> {
        umbra_dark_mode::toggle_mode()
    }

    fn press_action_text(&self) -> &'static str {
        if self.dark {
            "switch apps and the desktop to the light theme"
        } else {
            "switch apps and the desktop to the dark theme"
        }
    }
}
