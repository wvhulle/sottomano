pub mod alert_sound_indicator;
mod do_not_disturb_indicator;
pub mod pending_notifications_indicator;

use iced::Task;

use self::{
    alert_sound_indicator::AlertSound, pending_notifications_indicator::NotificationStatus,
};
use crate::{
    background_work::logged_failure_task,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
            IndicatorListing,
        },
        indicator_group_identity::IndicatorGroupFamily,
        sqlite_singleton_flag,
    },
    panel_menus::agenda_menu::quiet_schedule_group::QuietScheduleGroup,
};

#[derive(Debug, Clone, Copy)]
pub enum NotificationGroupMessage {
    Expansion(IndicatorGroupExpansionMessage),
    ToggleSound,
    ToggleDoNotDisturb,
    SoundLoaded(AlertSound),
}

pub enum Action {
    Run(Task<NotificationGroupMessage>),
    ToggleDoNotDisturb,
}

pub struct NotificationGroup {
    expansion: IndicatorGroupExpansion,
    pub alert_sound: AlertSound,
}

impl Default for NotificationGroup {
    fn default() -> Self {
        Self {
            expansion: IndicatorGroupExpansion::default(),
            alert_sound: AlertSound::Audible,
        }
    }
}

impl NotificationGroup {
    pub fn load() -> Task<NotificationGroupMessage> {
        Task::perform(
            sqlite_singleton_flag::load(AlertSound::Audible),
            NotificationGroupMessage::SoundLoaded,
        )
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

    pub fn group_content<'view>(
        &'view self,
        status: NotificationStatus,
        quiet_schedule: &'view QuietScheduleGroup,
    ) -> IndicatorGroupContent<'view> {
        let headline_indicator = status
            .indicator()
            .with_headline_glyph(Phosphor::Tray(Weight::Regular).into());
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::Notification,
            headline_indicator,
            &self.expansion,
            NotificationGroupMessage::Expansion,
            IndicatorListing::Listed(Vec::from([
                status.indicator(),
                Self::do_not_disturb_row(status.quiet_phase, quiet_schedule),
                self.alert_sound_row(),
            ])),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{NotificationGroup, NotificationGroupMessage, alert_sound_indicator::AlertSound};

    #[test]
    fn a_loaded_sound_setting_reads_back_unchanged() {
        let mut notification_group = NotificationGroup::default();
        let _ =
            notification_group.update(NotificationGroupMessage::SoundLoaded(AlertSound::Silent));
        assert_eq!(notification_group.alert_sound, AlertSound::Silent);
    }
}
