use iced::Task;

use crate::{
    background_work::logged_failure_task,
    group_items::{
        group_item::GroupItem,
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
        toggle_control::{IconToggle, SwitchPosition, SwitchPresses},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widget_groups::{
        group_summary::GroupSummary,
        quiet_schedule_group::{QuietScheduleGroup, quiet_window::QuietPhase},
        sqlite_singleton_flag::{self, SingletonFlag, SingletonFlagState},
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
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

    pub fn group_content<'a>(
        &'a self,
        status: NotificationStatus,
        quiet_schedule: &'a QuietScheduleGroup,
    ) -> WidgetGroupContent<'a> {
        let quiet_phase = status.quiet_phase;
        let group_summary = GroupSummary::summarising(
            Phosphor::Tray(Weight::Regular).into(),
            GroupItem::reading(status.reading_row()),
        );
        let group_summary = group_summary;
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Notification,
            group_summary,
            &self.expansion,
            NotificationGroupMessage::Expansion,
            [
                GroupItem::reading(do_not_disturb_reading_row(DoNotDisturbStatus {
                    quiet_phase,
                }))
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
                })),
                GroupItem::toggle(
                    sound_toggle(self.alert_sound),
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

impl NotificationStatus {
    fn reading_row(self) -> ReadingRow<'static> {
        let glyph = match self.quiet_phase {
            QuietPhase::Active => Phosphor::ChatCircleSlash(Weight::Regular).into(),
            QuietPhase::Inactive => Phosphor::ChatCircle(Weight::Regular).into(),
        };
        let state = match self.quiet_phase {
            QuietPhase::Active => ReadingDisplayState::Off,
            QuietPhase::Inactive if self.live > 0 => ReadingDisplayState::On,
            QuietPhase::Inactive => ReadingDisplayState::Idle,
        };
        let badge = match self.live + self.held {
            0 => ReadoutBadge::Unbadged,
            count => ReadoutBadge::Counted(count),
        };
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Unlabeled,
                gauge: ReadoutGauge::Ungauged,
                badge,
            },
            ..ReadingRow::glyph_only(
                "Pending notifications",
                glyph,
                state,
                Box::new(move |_| self.hover_card()),
            )
        }
    }

    fn hover_card(self) -> Vec<Section> {
        if self.held > 0 {
            vec![Section::new(
                "Counts",
                dictionary_table([LabeledValue::new(
                    "Held until do not disturb ends",
                    Quantity::count(self.held),
                )]),
            )]
        } else {
            vec![]
        }
    }
}

#[derive(Clone, Copy)]
struct DoNotDisturbStatus {
    quiet_phase: QuietPhase,
}

fn do_not_disturb_reading_row(status: DoNotDisturbStatus) -> ReadingRow<'static> {
    ReadingRow::glyph_only(
        "Do not disturb",
        Phosphor::Prohibit(Weight::Regular).into(),
        ReadingDisplayState::from(status.quiet_phase),
        Box::new(|_| Vec::new()),
    )
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
