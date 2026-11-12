use iced::Task;

use crate::{
    background_work::logged_failure_task,
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        group_item_reading::GroupItemReading,
        reading_display_state::ReadingDisplayState,
        toggle_control::{IconToggle, SwitchPosition, SwitchPresses},
    },
    hover_cards::{
        HoverTarget,
        hover_card_body::{
            HoverCardBody,
            hover_card_source_delivery::{DeliveryTiming, HoverCard},
        },
    },
    widget_groups::{
        quiet_schedule_group::{QuietScheduleTarget, quiet_window::QuietPhase},
        sqlite_singleton_flag::{self, SingletonFlag, SingletonFlagState},
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
    SoundLoaded(AlertSound),
}

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
    alert_sound: AlertSound,
}

impl Default for NotificationGroup {
    fn default() -> Self {
        Self {
            expansion: WidgetGroupExpansion::default(),
            alert_sound: AlertSound::Audible,
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
    pub fn load() -> Task<NotificationGroupMessage> {
        Task::perform(
            sqlite_singleton_flag::load(AlertSound::Audible),
            NotificationGroupMessage::SoundLoaded,
        )
    }

    #[must_use]
    pub fn alert_sound(&self) -> AlertSound {
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
                self.alert_sound = self.alert_sound.toggled();
                Action::Run(logged_failure_task(
                    "singleton flag: save".to_owned(),
                    sqlite_singleton_flag::save(self.alert_sound),
                ))
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
        let quiet_phase = status.quiet_phase;
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Notification,
            &self.expansion,
            NotificationGroupMessage::Expansion,
            [
                GroupItem::reading(status, NotificationTarget::Status),
                GroupItem::reading(
                    DoNotDisturbStatus { quiet_phase },
                    QuietScheduleTarget::DoNotDisturb,
                )
                .on_switch_press(
                    quiet_phase,
                    match quiet_phase {
                        QuietPhase::Active => "end do not disturb now",
                        QuietPhase::Inactive => "start do not disturb now",
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
    pub quiet_phase: QuietPhase,
    pub live: usize,
    pub held: usize,
}

impl GroupItemReading for NotificationStatus {
    fn name(&self) -> &'static str {
        "Notifications"
    }

    fn icon(&self) -> IconGlyph {
        match self.quiet_phase {
            QuietPhase::Active => Phosphor::ChatCircleSlash(Weight::Regular).into(),
            QuietPhase::Inactive => Phosphor::ChatCircle(Weight::Regular).into(),
        }
    }

    fn badge(&self) -> Option<usize> {
        Some(self.live + self.held).filter(|count| *count > 0)
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match self.quiet_phase {
            QuietPhase::Active => ReadingDisplayState::Off,
            QuietPhase::Inactive if self.live > 0 => ReadingDisplayState::On,
            QuietPhase::Inactive => ReadingDisplayState::Idle,
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
    quiet_phase: QuietPhase,
}

impl GroupItemReading for DoNotDisturbStatus {
    fn name(&self) -> &'static str {
        "Do not disturb"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Prohibit(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::from(self.quiet_phase)
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([])
    }
}

#[cfg(test)]
mod tests {
    use super::{AlertSound, NotificationGroup, NotificationGroupMessage};

    #[test]
    fn a_loaded_sound_setting_reads_back_unchanged() {
        let mut notification_group = NotificationGroup::default();
        let _ =
            notification_group.update(NotificationGroupMessage::SoundLoaded(AlertSound::Silent));
        assert_eq!(notification_group.alert_sound(), AlertSound::Silent);
    }
}
