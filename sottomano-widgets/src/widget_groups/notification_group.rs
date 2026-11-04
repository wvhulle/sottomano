use iced::Task;

use crate::{
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        group_item_reading::GroupItemReading,
        reading_display_state::ReadingDisplayState,
        toggle_control::{IconToggle, SwitchPresses},
    },
    hover_cards::{
        HoverTarget,
        hover_card_body::{
            HoverCardBody,
            hover_card_source_delivery::{DeliveryTiming, HoverCard},
        },
    },
    widget_groups::{
        quiet_schedule_group::QuietScheduleTarget,
        sqlite_singleton_flag::{self, SingletonFlag},
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

#[derive(Debug, Clone, Copy)]
pub enum NotificationGroupMessage {
    Expansion(WidgetGroupExpansionMessage),
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
    expansion: WidgetGroupExpansion,
    alert_sound: bool,
}

impl Default for NotificationGroup {
    fn default() -> Self {
        Self {
            expansion: WidgetGroupExpansion::default(),
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
            sqlite_singleton_flag::load(SingletonFlag::NotificationSound, true),
            NotificationGroupMessage::SoundLoaded,
        )
    }

    #[must_use]
    pub fn alert_sound_enabled(&self) -> bool {
        self.alert_sound
    }

    #[must_use]
    pub fn hover_card_for(
        &self,
        target: NotificationTarget,
        status: NotificationStatus,
        clock: Clock,
    ) -> HoverCard {
        match target {
            NotificationTarget::Status => {
                HoverCard::sourced(status.hover_card(clock), DeliveryTiming::LivePush)
            }
            NotificationTarget::Sound => sound_toggle(self.alert_sound).switch_hover_card(clock),
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
                    Task::future(sqlite_singleton_flag::save(
                        SingletonFlag::NotificationSound,
                        self.alert_sound,
                    ))
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
    ) -> WidgetGroupContent<'_, NotificationGroupMessage> {
        let quiet = status.quiet;
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Notification,
            &self.expansion,
            NotificationGroupMessage::Expansion,
            [
                GroupItem::reading(status, NotificationTarget::Status),
                GroupItem::reading(
                    DoNotDisturbStatus { quiet },
                    QuietScheduleTarget::DoNotDisturb,
                )
                .on_switch_press(
                    quiet,
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

impl GroupItemReading for NotificationStatus {
    fn name(&self) -> &'static str {
        "Notifications"
    }

    fn icon(&self) -> IconGlyph {
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

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        if self.quiet {
            ReadingDisplayState::Off
        } else if self.live > 0 {
            ReadingDisplayState::On
        } else {
            ReadingDisplayState::Idle
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        if self.held > 0 {
            HoverCardBody::dictionary([LabeledValue::new(
                "Held until do not disturb ends",
                Quantity::count(self.held),
            )])
        } else {
            HoverCardBody::sections([])
        }
    }
}

#[derive(Clone, Copy)]
struct DoNotDisturbStatus {
    quiet: bool,
}

impl GroupItemReading for DoNotDisturbStatus {
    fn name(&self) -> &'static str {
        "Do not disturb"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Prohibit(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::of_toggle(self.quiet)
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([])
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
