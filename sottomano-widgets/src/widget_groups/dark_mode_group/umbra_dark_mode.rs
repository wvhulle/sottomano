use sottomano_sources::{
    system_protocol_clients::shell_command_output,
    umbra::{self, Umbra},
};

use crate::widgets::{
    bar_widget::{BarWidget, Glyph},
    phosphor::{Phosphor, Weight},
    toggle_control::ToggleableSource,
    tooltip_body::TooltipBody,
};

impl BarWidget for Umbra {
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

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([])
    }
}

impl ToggleableSource for Umbra {
    fn toggled_value(&self) -> Self {
        Umbra { dark: !self.dark }
    }

    fn apply_toggle(&self) -> Result<(), shell_command_output::Error> {
        umbra::toggle_mode()
    }

    fn press_action_text(&self) -> &'static str {
        if self.dark {
            "switch apps and the desktop to the light theme"
        } else {
            "switch apps and the desktop to the dark theme"
        }
    }
}
