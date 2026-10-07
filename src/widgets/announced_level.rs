use std::time::Duration;

use uom::si::{f64::Ratio, ratio};

use crate::{
    data::{
        notifications::model::{ExpireTimeout, NotificationData, Progress},
        severity::Severity,
    },
    desktop_shell::hover_tooltips::HoverTarget,
    widgets::bar_widget::Glyph,
};

#[derive(Debug, Clone, PartialEq)]
pub struct AnnouncedLevel {
    pub summary_text: &'static str,
    pub readout_text: String,
    pub fill: Ratio,
    pub muted: bool,
    pub external_change_count: u64,
}

impl AnnouncedLevel {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn fill_percent(&self) -> u8 {
        self.fill.get::<ratio::percent>().round().clamp(0.0, 100.0) as u8
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LevelReading {
    pub hover_target: HoverTarget,
    pub glyph: Glyph,
    pub level: AnnouncedLevel,
}

impl LevelReading {
    pub fn toast_notification_data(&self, toast_id: u32, lifetime: Duration) -> NotificationData {
        let body = if self.level.muted {
            format!("Muted · {}", self.level.readout_text)
        } else {
            self.level.readout_text.clone()
        };
        NotificationData {
            id: toast_id,
            summary: self.level.summary_text.to_owned(),
            body: Some(body),
            progress: Progress::Percent(self.level.fill_percent()),
            severity: Severity::Low,
            suppress_sound: true,
            expire_timeout: ExpireTimeout::After(lifetime),
            ..NotificationData::default()
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

    pub fn level(&self, fill: Ratio, muted: bool, readout_text: String) -> Option<AnnouncedLevel> {
        self.external_change_count
            .map(|external_change_count| AnnouncedLevel {
                summary_text: self.summary_text,
                readout_text,
                fill,
                muted,
                external_change_count,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count_after(reports: &[bool]) -> Option<u64> {
        let mut announcement = LevelAnnouncement::new("level");
        for changed in reports {
            announcement.device_reported(*changed);
        }
        announcement
            .level(Ratio::new::<ratio::ratio>(0.5), false, String::new())
            .map(|level| level.external_change_count)
    }

    #[test]
    fn nothing_is_announced_before_the_device_first_reports() {
        assert_eq!(count_after(&[]), None);
    }

    #[test]
    fn the_first_report_only_seeds_the_count() {
        assert_eq!(count_after(&[true]), Some(0));
    }

    #[test]
    fn only_changes_outside_the_bar_are_counted() {
        assert_eq!(count_after(&[false, true, false, true]), Some(2));
    }
}
