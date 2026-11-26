use super::{QuietScheduleGroup, QuietScheduleMessage};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        toggle_control::{IconToggle, SwitchPresses},
    },
};

impl QuietScheduleGroup {
    pub(super) fn follow_schedule_row(&self) -> GroupIndicator<'_> {
        GroupIndicator::toggle(
            self.follow_schedule_toggle(),
            QuietScheduleMessage::ToggleFollowsSchedule,
        )
    }

    fn follow_schedule_toggle(&self) -> IconToggle {
        IconToggle::new(
            "Follow schedule",
            Phosphor::Repeat(Weight::Regular),
            self.schedule_adherence,
            SwitchPresses {
                turning_on: "start and end do not disturb on the daily schedule",
                turning_off: "stop following the daily quiet schedule",
            },
        )
    }
}
