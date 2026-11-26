use super::{NotificationGroup, NotificationGroupMessage};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{group_indicator::GroupIndicator, indicator_state::IndicatorState},
    panel_menus::agenda_menu::quiet_schedule_group::{
        QuietScheduleGroup, quiet_window::QuietPhase,
    },
};

#[derive(Clone, Copy)]
struct DoNotDisturbStatus {
    quiet_phase: QuietPhase,
}

fn do_not_disturb_indicator<'indicator>(status: DoNotDisturbStatus) -> GroupIndicator<'indicator> {
    GroupIndicator::glyph_only(
        "Do not disturb",
        Phosphor::Prohibit(Weight::Regular).into(),
        IndicatorState::from(status.quiet_phase),
    )
    .with_hover_card(Box::new(|_| Vec::new()))
}

impl NotificationGroup {
    pub(super) fn do_not_disturb_row(
        quiet_phase: QuietPhase,
        quiet_schedule: &QuietScheduleGroup,
    ) -> GroupIndicator<'_> {
        do_not_disturb_indicator(DoNotDisturbStatus { quiet_phase })
            .on_switch_press(
                quiet_phase,
                match quiet_phase {
                    QuietPhase::Active => "end do not disturb now",
                    QuietPhase::Inactive => "start do not disturb now",
                },
                NotificationGroupMessage::ToggleDoNotDisturb,
            )
            .with_hover_card(Box::new(move |clock| {
                quiet_schedule.do_not_disturb_hover_card(clock)
            }))
    }
}
