use sottomano_desktop_sinks::screen_brightness_color::clight_calibration_writes;
use sottomano_desktop_sources::{
    SnapshotSource,
    screen_brightness_color::clight_auto_brightness::{AutoBrightness, BrightnessCalibration},
    system_protocol_clients::dbus_desktop_buses::BusReadError,
};

use super::{MonitorBusRequest, MonitorGroup, MonitorInteraction, MonitorSource};
use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_group_content::IndicatorListing,
        indicator_state::IndicatorState,
        toggle_control::{SwitchPosition, ToggleableSource},
    },
};

const AUTO_BRIGHTNESS_GLYPH: Phosphor = Phosphor::Eye(Weight::Regular);

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

impl MonitorGroup {
    pub(super) fn auto_brightness_listing(&self) -> IndicatorListing<'_> {
        self.clight.sample.indicator_listing(
            |held_snapshot| {
                self.wired_auto_brightness_indicator(auto_brightness_indicator(held_snapshot))
                    .on_switch_press(
                        held_snapshot.switch_position(),
                        held_snapshot.press_action_text(),
                        MonitorInteraction::AutoBrightnessPressed,
                    )
                    .when_shown()
            },
            AUTO_BRIGHTNESS_GLYPH.into(),
        )
    }

    fn wired_auto_brightness_indicator<'view>(
        &'view self,
        brightness_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        brightness_indicator
            .with_hover_card(Box::new(move |_| self.clight.hover_card()))
            .refreshed_by(MonitorInteraction::OnBuses(MonitorBusRequest::Refresh(
                MonitorSource::AutoBrightness,
            )))
    }
}
