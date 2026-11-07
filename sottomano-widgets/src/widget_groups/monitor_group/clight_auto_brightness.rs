use sottomano_desktop_sinks::screen_brightness_color::clight_calibration_writes;
use sottomano_desktop_sources::{
    screen_brightness_color::clight_auto_brightness::{AutoBrightness, BrightnessCalibration},
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
        match self.calibration {
            BrightnessCalibration::FromWebcam => Phosphor::Eye(Weight::Regular).into(),
            BrightnessCalibration::Fixed => Phosphor::EyeSlash(Weight::Regular).into(),
        }
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match self.calibration {
            BrightnessCalibration::FromWebcam => ReadingDisplayState::On,
            BrightnessCalibration::Fixed => ReadingDisplayState::Off,
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([])
    }
}

impl ToggleableSource for AutoBrightness {
    fn toggled_value(&self) -> Self {
        AutoBrightness {
            calibration: self.calibration.flipped(),
        }
    }

    fn apply_toggle(&self) -> Result<(), dbus_desktop_buses::Error> {
        clight_calibration_writes::flip()
    }

    fn switched_on(&self) -> bool {
        self.calibration == BrightnessCalibration::FromWebcam
    }

    fn press_action_text(&self) -> &'static str {
        match self.calibration {
            BrightnessCalibration::FromWebcam => {
                "stop setting brightness from webcam light; keep the current level"
            }
            BrightnessCalibration::Fixed => "set brightness from the light the webcam sees",
        }
    }
}
