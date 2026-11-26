use std::borrow::Cow;

use super::{ColorTemperatureBusRequest, ColorTemperatureGroup, ColorTemperatureInteraction};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        toggle_control::SwitchPresses,
        unit_switch::{ServiceReadiness, UnitSwitchFace},
    },
};

pub const SUN_FOLLOW: UnitSwitchFace = UnitSwitchFace {
    name: Cow::Borrowed("Sun follow"),
    icon: Phosphor::SunHorizon(Weight::Regular),
    presses: SwitchPresses {
        turning_on: "warm the screen at sunset and cool it at sunrise again",
        turning_off: "stop shifting screen colour with the sun; keep the current temperature",
    },
};

impl ColorTemperatureGroup {
    pub(super) fn sun_follow_rows(&self) -> Vec<GroupIndicator<'_>> {
        self.auto
            .switch_indicator(ServiceReadiness::Serving)
            .map(|switch_indicator| {
                switch_indicator
                    .wired_switch_indicator(ColorTemperatureInteraction::AutoPressed)
                    .with_hover_card(Box::new(move |_| {
                        self.auto.hover_card(ServiceReadiness::Serving)
                    }))
                    .refreshed_by(ColorTemperatureInteraction::OnBuses(
                        ColorTemperatureBusRequest::Refresh,
                    ))
            })
            .into_iter()
            .collect()
    }
}
