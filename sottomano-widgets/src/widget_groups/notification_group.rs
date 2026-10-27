use iced::Task;

use crate::{
    clock::Clock,
    hover_tooltips::HoverTarget,
    widget_groups::{
        quiet_schedule_group::QuietScheduleTarget,
        sqlite_shell_settings_row::{self, ShellSetting},
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::GroupItem,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        toggle_control::{IconToggle, SwitchPresses},
        tooltip_body::{DeliveryTiming, KeyValue, Tooltip, TooltipBody},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
        widget_state::WidgetState,
    },
};

#[derive(Debug, Clone, Copy)]
pub enum NotificationGroupMessage {
    Expansion(ExpansionMessage),
    ToggleSound,
    ToggleDoNotDisturb,
    SoundLoaded(bool),
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

pub enum Action {
    Run(Task<NotificationGroupMessage>),
    ToggleDoNotDisturb,
}

pub struct NotificationGroup {
    expansion: GroupExpansion,
    alert_sound: bool,
}

impl Default for NotificationGroup {
    fn default() -> Self {
        Self {
            expansion: GroupExpansion::default(),
            alert_sound: true,
        }
    }
}

fn sound_toggle(alert_sound: bool) -> IconToggle {
    IconToggle::new(
        "Notification sound",
        if alert_sound {
            Phosphor::Bell(Weight::Regular)
        } else {
            Phosphor::BellSlash(Weight::Regular)
        },
        alert_sound,
        SwitchPresses {
            turning_on: "play a sound with each notification",
            turning_off: "show notifications silently",
        },
    )
}

impl NotificationGroup {
    pub fn load() -> Task<NotificationGroupMessage> {
        Task::perform(
            sqlite_shell_settings_row::load_stored_flag(
                "shell settings: load sound failed",
                "sound",
                true,
            ),
            NotificationGroupMessage::SoundLoaded,
        )
    }

    #[must_use]
    pub fn alert_sound_enabled(&self) -> bool {
        self.alert_sound
    }

    #[must_use]
    pub fn tooltip_for(
        &self,
        target: NotificationTarget,
        status: NotificationStatus,
        clock: Clock,
    ) -> Tooltip {
        match target {
            NotificationTarget::Status => {
                Tooltip::sourced(status.tooltip(clock), DeliveryTiming::LivePush)
            }
            NotificationTarget::Sound => sound_toggle(self.alert_sound).switch_tooltip(clock),
        }
    }

    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn update(&mut self, message: NotificationGroupMessage) -> Action {
        match message {
            NotificationGroupMessage::Expansion(menu) => {
                self.expansion.update(menu);
                Action::Run(Task::none())
            }
            NotificationGroupMessage::ToggleSound => {
                self.alert_sound = !self.alert_sound;
                Action::Run(
                    Task::future(sqlite_shell_settings_row::save(ShellSetting::Sound(
                        self.alert_sound,
                    )))
                    .discard(),
                )
            }
            NotificationGroupMessage::SoundLoaded(alert_sound) => {
                self.alert_sound = alert_sound;
                Action::Run(Task::none())
            }
            NotificationGroupMessage::ToggleDoNotDisturb => Action::ToggleDoNotDisturb,
        }
    }

    pub fn group_content(
        &self,
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
                    sound_toggle(self.alert_sound),
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
                Quantity::count(self.held),
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

#[cfg(test)]
mod tests {
    use super::{NotificationGroup, NotificationGroupMessage};

    #[test]
    fn a_loaded_sound_setting_reads_back_unchanged() {
        let mut notification_group = NotificationGroup::default();
        let _ = notification_group.update(NotificationGroupMessage::SoundLoaded(false));
        assert!(!notification_group.alert_sound_enabled());
    }
}
