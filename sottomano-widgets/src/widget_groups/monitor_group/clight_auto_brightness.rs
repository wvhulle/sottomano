use sottomano_sources::{
    screen_brightness_color::clight_auto_brightness::{self, AutoBrightness},
    system_protocol_clients::dbus_desktop_buses,
};

use crate::widgets::{
    bar_widget::{BarWidget, Glyph},
    phosphor::Phosphor,
    toggle_control::ToggleableSource,
    tooltip_body::TooltipBody,
    widget_state::WidgetState,
};

impl BarWidget for AutoBrightness {
    fn name(&self) -> &'static str {
        "Auto-brightness"
    }

    fn icon(&self) -> Glyph {
        if self.auto {
            Phosphor::Eye(self.state().weight()).into()
        } else {
            Phosphor::EyeSlash(self.state().weight()).into()
        }
    }

    fn state(&self) -> WidgetState {
        WidgetState::of_toggle(self.auto)
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([])
    }
}

impl ToggleableSource for AutoBrightness {
    fn toggled_value(&self) -> Self {
        AutoBrightness { auto: !self.auto }
    }

    fn apply_toggle(&self) -> Result<(), dbus_desktop_buses::Error> {
        clight_auto_brightness::flip()
    }

    fn press_action_text(&self) -> &'static str {
        if self.auto {
            "stop setting brightness from webcam light; keep the current level"
        } else {
            "set brightness from the light the webcam sees"
        }
    }
}
