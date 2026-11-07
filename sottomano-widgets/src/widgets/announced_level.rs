use std::time::Duration;

use sottomano_desktop_sources::desktop_notifications::notification_data::{
    ExpireTimeout, NotificationData, SoundHint,
};
use sottomano_toml_config::severity::Severity;
use uom::si::f64::Ratio;

use crate::{hover_cards::HoverTarget, widgets::icon_glyph::IconGlyph};

const LEVEL_TOAST_LIFETIME: Duration = Duration::from_millis(1_500);

#[derive(Debug, Clone, PartialEq)]
pub struct AnnouncedLevel {
    pub summary_text: &'static str,
    pub body_text: String,
    pub fill: Ratio,
    pub external_change_count: u64,
}

#[derive(Debug, Clone)]
pub struct LevelReading<Message> {
    pub hover_target: HoverTarget,
    pub glyph: IconGlyph,
    pub level: AnnouncedLevel,
    pub on_body_clicked: Message,
}

impl<Message> LevelReading<Message> {
    pub fn change_since(&self, previous: &[LevelReading<Message>]) -> LevelReadingChange {
        if previous.iter().any(|seen| {
            seen.hover_target == self.hover_target
                && seen.level.external_change_count != self.level.external_change_count
        }) {
            LevelReadingChange::Changed
        } else {
            LevelReadingChange::Unchanged
        }
    }

    pub fn toast_notification_data(&self, toast_id: u32) -> NotificationData {
        NotificationData {
            app_name: None,
            body: Some(self.level.body_text.clone()),
            progress: Some(self.level.fill),
            severity: Severity::Low,
            sound_hint: SoundHint::Suppressed,
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
    #[must_use]
    pub fn new(summary_text: &'static str) -> Self {
        Self {
            summary_text,
            external_change_count: None,
        }
    }

    pub fn device_reported(&mut self, change_origin: LevelChangeOrigin) {
        let external_increment = match change_origin {
            LevelChangeOrigin::OutsidePanel => 1,
            LevelChangeOrigin::FromPanel => 0,
        };
        self.external_change_count = Some(
            self.external_change_count
                .map_or(0, |count| count + external_increment),
        );
    }

    #[must_use]
    pub fn level(&self, fill: Ratio, body_text: String) -> Option<AnnouncedLevel> {
        self.external_change_count
            .map(|external_change_count| AnnouncedLevel {
                summary_text: self.summary_text,
                body_text,
                fill,
                external_change_count,
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelChangeOrigin {
    OutsidePanel,
    FromPanel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelReadingChange {
    Changed,
    Unchanged,
}

#[cfg(test)]
mod tests {
    use uom::si::ratio::percent;

    use super::*;
    use crate::{
        widget_groups::audio_group::ControlKind,
        widgets::phosphor::{Phosphor, Weight},
    };

    fn count_after(reports: &[LevelChangeOrigin]) -> Option<u64> {
        let mut announcement = LevelAnnouncement::new("level");
        reports
            .iter()
            .for_each(|change_origin| announcement.device_reported(*change_origin));
        announcement
            .level(Ratio::new::<percent>(50.0), String::new())
            .map(|level| level.external_change_count)
    }

    fn reading(kind: ControlKind, external_change_count: u64) -> LevelReading<()> {
        LevelReading {
            hover_target: kind.into(),
            glyph: Phosphor::SpeakerHigh(Weight::Regular).into(),
            level: AnnouncedLevel {
                summary_text: "Speaker",
                body_text: "50%".to_owned(),
                fill: Ratio::new::<percent>(50.0),
                external_change_count,
            },
            on_body_clicked: (),
        }
    }

    #[test]
    fn only_changes_outside_the_panel_after_the_first_report_are_counted() {
        assert_eq!(count_after(&[]), None);
        assert_eq!(count_after(&[LevelChangeOrigin::OutsidePanel]), Some(0));
        assert_eq!(
            count_after(&[
                LevelChangeOrigin::FromPanel,
                LevelChangeOrigin::OutsidePanel,
                LevelChangeOrigin::FromPanel,
                LevelChangeOrigin::OutsidePanel,
            ]),
            Some(2)
        );
    }

    #[test]
    fn a_level_is_announced_only_when_its_seen_count_changed() {
        let seen = [
            reading(ControlKind::Sink, 3),
            reading(ControlKind::Source, 1),
        ];
        assert_eq!(
            reading(ControlKind::Sink, 3).change_since(&[]),
            LevelReadingChange::Unchanged
        );
        assert_eq!(
            reading(ControlKind::Sink, 3).change_since(&seen),
            LevelReadingChange::Unchanged
        );
        assert_eq!(
            reading(ControlKind::Sink, 4).change_since(&seen),
            LevelReadingChange::Changed
        );
    }
}
