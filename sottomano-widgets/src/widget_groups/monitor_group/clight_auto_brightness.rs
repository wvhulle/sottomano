use sottomano_sources::{
    screen_brightness_color::clight_auto_brightness::{self, AutoBrightness},
    system_protocol_clients::dbus_desktop_buses,
};

use crate::{
    clock::Clock,
    widgets::{
        group_item_reading::GroupItemReading,
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        reading_display_state::ReadingDisplayState,
        toggle_control::ToggleableSource,
        tooltip_body::TooltipBody,
    },
};

impl GroupItemReading for AutoBrightness {
    fn name(&self) -> &'static str {
        "Auto-brightness"
    }

    fn icon(&self) -> IconGlyph {
        if self.auto {
            Phosphor::Eye(Weight::Regular).into()
        } else {
            Phosphor::EyeSlash(Weight::Regular).into()
        }
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::of_toggle(self.auto)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
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
