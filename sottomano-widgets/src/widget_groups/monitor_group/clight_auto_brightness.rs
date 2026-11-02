use sottomano_sources::{
    screen_brightness_color::clight_auto_brightness::{self, AutoBrightness},
    system_protocol_clients::dbus_desktop_buses,
};

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading, reading_display_state::ReadingDisplayState,
        toggle_control::ToggleableSource,
    },
    hover_cards::hover_card_body::HoverCardBody,
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
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

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([])
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
