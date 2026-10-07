use std::time::Duration;

use crate::{
    data::{
        notifications::model::{ExpireTimeout, NotificationData, Progress},
        severity::Severity,
    },
    desktop_shell::hover_tooltips::HoverTarget,
    widgets::bar_widget::Glyph,
};

const LEVEL_TOAST_LIFETIME: Duration = Duration::from_millis(1_500);

#[derive(Debug, Clone, PartialEq)]
pub struct AnnouncedLevel {
    pub summary_text: &'static str,
    pub body_text: String,
    pub percent: u8,
    pub external_change_count: u64,
}

#[derive(Debug, Clone)]
pub struct LevelReading<Message> {
    pub hover_target: HoverTarget,
    pub glyph: Glyph,
    pub level: AnnouncedLevel,
    pub on_body_clicked: Message,
}

impl<Message> LevelReading<Message> {
    pub fn map_messages<Lifted>(self, lift: impl Fn(Message) -> Lifted) -> LevelReading<Lifted> {
        LevelReading {
            hover_target: self.hover_target,
            glyph: self.glyph,
            level: self.level,
            on_body_clicked: lift(self.on_body_clicked),
        }
    }

    pub fn changed_since(&self, previous: &[LevelReading<Message>]) -> bool {
        previous.iter().any(|seen| {
            seen.hover_target == self.hover_target
                && seen.level.external_change_count != self.level.external_change_count
        })
    }

    pub fn toast_notification_data(&self, toast_id: u32) -> NotificationData {
        NotificationData {
            app_name: None,
            body: Some(self.level.body_text.clone()),
            progress: Progress::Percent(self.level.percent),
            severity: Severity::Low,
            suppress_sound: true,
            expire_timeout: ExpireTimeout::After(LEVEL_TOAST_LIFETIME),
            ..NotificationData::from_shell(toast_id, self.level.summary_text)
        }
    }
}

pub struct LevelAnnouncement {
    summary_text: &'static str,
    external_change_count: Option<u64>,
}

impl LevelAnnouncement {
    pub fn new(summary_text: &'static str) -> Self {
        Self {
            summary_text,
            external_change_count: None,
        }
    }

    pub fn device_reported(&mut self, changed_outside_the_bar: bool) {
        self.external_change_count = Some(
            self.external_change_count
                .map_or(0, |count| count + u64::from(changed_outside_the_bar)),
        );
    }

    pub fn level(&self, percent: u8, body_text: String) -> Option<AnnouncedLevel> {
        self.external_change_count
            .map(|external_change_count| AnnouncedLevel {
                summary_text: self.summary_text,
                body_text,
                percent,
                external_change_count,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::phosphor::{Phosphor, Weight};

    fn count_after(reports: &[bool]) -> Option<u64> {
        let mut announcement = LevelAnnouncement::new("level");
        reports
            .iter()
            .for_each(|changed| announcement.device_reported(*changed));
        announcement
            .level(50, String::new())
            .map(|level| level.external_change_count)
    }

    fn reading(hover_target: HoverTarget, external_change_count: u64) -> LevelReading<()> {
        LevelReading {
            hover_target,
            glyph: Phosphor::SpeakerHigh(Weight::Regular).into(),
            level: AnnouncedLevel {
                summary_text: "Speaker",
                body_text: "50%".to_owned(),
                percent: 50,
                external_change_count,
            },
            on_body_clicked: (),
        }
    }

    #[test]
    fn only_changes_outside_the_bar_after_the_first_report_are_counted() {
        assert_eq!(count_after(&[]), None);
        assert_eq!(count_after(&[true]), Some(0));
        assert_eq!(count_after(&[false, true, false, true]), Some(2));
    }

    #[test]
    fn a_level_is_announced_only_when_its_seen_count_changed() {
        let seen = [
            reading(HoverTarget::AudioSink, 3),
            reading(HoverTarget::AudioSource, 1),
        ];
        assert!(!reading(HoverTarget::AudioSink, 3).changed_since(&[]));
        assert!(!reading(HoverTarget::AudioSink, 3).changed_since(&seen));
        assert!(reading(HoverTarget::AudioSink, 4).changed_since(&seen));
    }
}
