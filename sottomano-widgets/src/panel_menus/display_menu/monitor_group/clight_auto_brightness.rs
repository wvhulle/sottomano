use sottomano_desktop_sinks::screen_brightness_color::clight_calibration_writes;
use sottomano_desktop_sources::{
    SnapshotSource,
    screen_brightness_color::clight_auto_brightness::{AutoBrightness, BrightnessCalibration},
    system_protocol_clients::dbus_desktop_buses::BusReadError,
};

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

pub fn auto_brightness_indicator(brightness: &AutoBrightness) -> GroupIndicator<'_> {
    let (glyph, state): (IconGlyph, _) = match brightness.calibration {
        BrightnessCalibration::FromWebcam => {
            (Phosphor::Eye(Weight::Regular).into(), IndicatorState::On)
        }
        BrightnessCalibration::Fixed => (
            Phosphor::EyeSlash(Weight::Regular).into(),
            IndicatorState::Off,
        ),
    };
    GroupIndicator::glyph_only(AutoBrightness::NAME, glyph, state)
        .with_hover_card(Box::new(|_| Vec::new()))
}

impl ToggleableSource for AutoBrightness {
    fn toggled_value(&self) -> Self {
        AutoBrightness {
            calibration: self.calibration.flipped(),
        }
    }

    fn apply_toggle(&self) -> impl Future<Output = Result<(), BusReadError>> + Send + 'static {
        clight_calibration_writes::flip()
    }

    fn switch_position(&self) -> SwitchPosition {
        match self.calibration {
            BrightnessCalibration::FromWebcam => SwitchPosition::On,
            BrightnessCalibration::Fixed => SwitchPosition::Off,
        }
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
