use super::{NotificationGroup, NotificationGroupMessage};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        sqlite_singleton_flag::{SingletonFlag, SingletonFlagState},
        toggle_control::{IconToggle, SwitchPosition, SwitchPresses},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertSound {
    Audible,
    Silent,
}

impl AlertSound {
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            AlertSound::Audible => AlertSound::Silent,
            AlertSound::Silent => AlertSound::Audible,
        }
    }
}

impl SingletonFlagState for AlertSound {
    const STORED_FLAG: SingletonFlag = SingletonFlag::NotificationSound;
    const STORED_AS_SET: Self = AlertSound::Audible;
    const STORED_AS_CLEARED: Self = AlertSound::Silent;
}

impl From<AlertSound> for SwitchPosition {
    fn from(alert_sound: AlertSound) -> Self {
        match alert_sound {
            AlertSound::Audible => SwitchPosition::On,
            AlertSound::Silent => SwitchPosition::Off,
        }
    }
}

fn sound_toggle(alert_sound: AlertSound) -> IconToggle {
    IconToggle::new(
        "Notification sound",
        match alert_sound {
            AlertSound::Audible => Phosphor::Bell(Weight::Regular),
            AlertSound::Silent => Phosphor::BellSlash(Weight::Regular),
        },
        alert_sound,
        SwitchPresses {
            turning_on: "play a sound with each notification",
            turning_off: "show notifications silently",
        },
    )
}

impl NotificationGroup {
    pub(super) fn alert_sound_row(&self) -> GroupIndicator<'_> {
        GroupIndicator::toggle(
            sound_toggle(self.alert_sound),
            NotificationGroupMessage::ToggleSound,
        )
    }
}
