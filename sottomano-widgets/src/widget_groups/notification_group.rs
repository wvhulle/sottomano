use sottomano_state::NotificationSettings;

use crate::{
    clock::Clock,
    hover_tooltips::HoverTarget,
    widget_groups::quiet_schedule_group::QuietScheduleTarget,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        toggle_control::{IconToggle, SwitchPresses},
        tooltip_body::{Freshness, KeyValue, Tooltip, TooltipBody},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
        widget_state::WidgetState,
    },
};

#[derive(Debug, Clone, Copy)]
pub enum NotificationGroupMessage {
    Expansion(ExpansionMessage),
    ToggleSound,
    ToggleDoNotDisturb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationTarget {
    Status,
    Sound,
}

impl From<NotificationTarget> for HoverTarget {
    fn from(target: NotificationTarget) -> Self {
        HoverTarget::Notification(target)
    }
}

#[derive(Default)]
pub struct NotificationGroup {
    expansion: GroupExpansion,
}

fn sound_toggle(notification_settings: NotificationSettings) -> IconToggle {
    IconToggle::new(
        "Notification sound",
        if notification_settings.sound {
            Phosphor::Bell(Weight::Regular)
        } else {
            Phosphor::BellSlash(Weight::Regular)
        },
        notification_settings.sound,
        SwitchPresses {
            turning_on: "play a sound with each notification",
            turning_off: "show notifications silently",
        },
    )
}

impl NotificationGroup {
    #[must_use]
    pub fn tooltip_for(
        target: NotificationTarget,
        notification_settings: NotificationSettings,
        status: NotificationStatus,
        clock: Clock,
    ) -> Tooltip {
        match target {
            NotificationTarget::Status => Tooltip::sourced(status.tooltip(clock), Freshness::Live),
            NotificationTarget::Sound => sound_toggle(notification_settings).switch_tooltip(clock),
        }
    }

    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn update(&mut self, message: ExpansionMessage) {
        self.expansion.update(message);
    }

    pub fn group_content(
        &self,
        notification_settings: NotificationSettings,
        status: NotificationStatus,
    ) -> GroupContent<'_, NotificationGroupMessage> {
        let quiet = status.quiet;
        GroupContent::new(
            &self.expansion,
            NotificationGroupMessage::Expansion,
            [
                GroupItem::reading(status, NotificationTarget::Status),
                GroupItem::reading(
                    DoNotDisturbStatus { quiet },
                    QuietScheduleTarget::DoNotDisturb,
                )
                .on_press(
                    if quiet {
                        "end do not disturb now"
                    } else {
                        "start do not disturb now"
                    },
                    NotificationGroupMessage::ToggleDoNotDisturb,
                ),
                GroupItem::toggle(
                    sound_toggle(notification_settings),
                    NotificationTarget::Sound,
                    NotificationGroupMessage::ToggleSound,
                ),
            ],
        )
    }
}

#[derive(Clone, Copy)]
pub struct NotificationStatus {
    pub quiet: bool,
    pub live: usize,
    pub held: usize,
}

impl BarWidget for NotificationStatus {
    fn name(&self) -> &'static str {
        "Notifications"
    }

    fn icon(&self) -> Glyph {
        if self.quiet {
            Phosphor::ChatCircleSlash(Weight::Regular).into()
        } else {
            Phosphor::ChatCircle(Weight::Regular).into()
        }
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.live + self.held)
            .ok()
            .filter(|count| *count > 0)
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        if self.quiet {
            WidgetState::Off
        } else if self.live > 0 {
            WidgetState::On
        } else {
            WidgetState::Idle
        }
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        if self.held > 0 {
            TooltipBody::dictionary([KeyValue::new(
                "Held until do not disturb ends",
                self.held.to_string(),
            )])
        } else {
            TooltipBody::sections([])
        }
    }
}

#[derive(Clone, Copy)]
struct DoNotDisturbStatus {
    quiet: bool,
}

impl BarWidget for DoNotDisturbStatus {
    fn name(&self) -> &'static str {
        "Do not disturb"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Prohibit(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        WidgetState::of_toggle(self.quiet)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([])
    }
}
