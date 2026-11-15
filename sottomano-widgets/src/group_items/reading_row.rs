use std::borrow::Cow;

use chrono::Locale;

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::{GroupItemReading, OngoingToastThreshold},
        reading_display_state::ReadingDisplayState,
        source_reading::{
            GroupItemIcon, IdentityOrdinal, ReadoutBadge, ReadoutGauge, ReadoutLabel, RowFace,
            RowReadout,
        },
    },
    hover_cards::hover_card_body::Section,
    widgets::{
        icon_glyph::IconGlyph,
        ongoing_toast::{LevelNotice, ReadingAlerts, StateToast, ToastBody, ToastProgress},
    },
};

pub type DescribeCard<'a> = Box<dyn Fn(Clock) -> Vec<Section> + 'a>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelVisibility {
    Shown,
    Hidden,
}

pub struct ReadingRow<'a> {
    pub face: RowFace,
    pub readout: RowReadout,
    pub alerts: ReadingAlerts,
    pub panel_visibility: PanelVisibility,
    pub card: DescribeCard<'a>,
}

impl<'a> ReadingRow<'a> {
    #[must_use]
    pub fn evaluated(reading: impl GroupItemReading + 'a, clock: Clock, locale: Locale) -> Self {
        let state = reading.state(clock);
        let glyph = reading.icon();
        let gauge = reading.gauge();
        let face = RowFace {
            name: reading.name(),
            icon: GroupItemIcon::IconGlyph { glyph, state },
            ordinal: match reading.identity_ordinal() {
                Some(ordinal) => IdentityOrdinal::Numbered(ordinal),
                None => IdentityOrdinal::Unnumbered,
            },
        };
        let readout = RowReadout {
            label: match reading.label(clock, locale) {
                Some(label) if !label.is_empty() => ReadoutLabel::Labeled(label),
                Some(_) | None => ReadoutLabel::Unlabeled,
            },
            gauge: match gauge {
                Some(fill) => ReadoutGauge::Gauged {
                    fill,
                    live: reading.live_gauge().unwrap_or(fill),
                },
                None => ReadoutGauge::Ungauged,
            },
            badge: match reading.badge() {
                Some(count) if count > 0 => ReadoutBadge::Counted(count),
                Some(_) | None => ReadoutBadge::Unbadged,
            },
        };
        let alerts = ReadingAlerts {
            state_toast: match reading.ongoing_toast_threshold() {
                OngoingToastThreshold::NeverNotify => StateToast::NeverNotify,
                OngoingToastThreshold::NotifyFromSeverity {
                    minimum_severity,
                    summary_text,
                } => StateToast::NotifyFromSeverity {
                    minimum_severity,
                    summary_text,
                    body: match reading.ongoing_toast_body(clock, locale) {
                        Some(body) => ToastBody::Text(body),
                        None => ToastBody::Bodiless,
                    },
                    progress: match gauge {
                        Some(fill) => ToastProgress::Measured(fill),
                        None => ToastProgress::Unmeasured,
                    },
                },
            },
            level: match reading.announced_level(locale) {
                Some(level) => LevelNotice::Announced {
                    level,
                    glyph: glyph.with_weight(state.weight()),
                },
                None => LevelNotice::Unannounced,
            },
            quota_windows: reading.quota_windows(clock.utc()),
        };
        Self {
            face,
            readout,
            alerts,
            panel_visibility: reading.panel_visibility(),
            card: Box::new(move |card_clock| reading.hover_card(card_clock)),
        }
    }

    #[must_use]
    pub fn glyph_only(
        name: impl Into<Cow<'static, str>>,
        glyph: IconGlyph,
        state: ReadingDisplayState,
        card: DescribeCard<'a>,
    ) -> Self {
        Self {
            face: RowFace {
                name: name.into(),
                icon: GroupItemIcon::IconGlyph { glyph, state },
                ordinal: IdentityOrdinal::Unnumbered,
            },
            readout: RowReadout {
                label: ReadoutLabel::Unlabeled,
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            alerts: ReadingAlerts::default(),
            panel_visibility: PanelVisibility::Shown,
            card,
        }
    }

    #[must_use]
    pub fn shown_state(&self) -> ReadingDisplayState {
        self.face.icon.display_state()
    }

    #[must_use]
    pub fn hover_card(&self, clock: Clock) -> Vec<Section> {
        (self.card)(clock)
    }

    #[must_use]
    pub fn in_state(self, state: ReadingDisplayState) -> Self {
        let icon = match self.face.icon {
            GroupItemIcon::IconGlyph { glyph, .. } => GroupItemIcon::IconGlyph { glyph, state },
            unchanged_icon @ (GroupItemIcon::Image(_) | GroupItemIcon::Absent) => unchanged_icon,
        };
        let level = match self.alerts.level {
            LevelNotice::Announced { level, glyph } => LevelNotice::Announced {
                level,
                glyph: glyph.with_weight(state.weight()),
            },
            LevelNotice::Unannounced => LevelNotice::Unannounced,
        };
        Self {
            face: RowFace { icon, ..self.face },
            alerts: ReadingAlerts {
                level,
                ..self.alerts
            },
            ..self
        }
    }
}

pub fn reading_hover_card(reading: impl GroupItemReading, clock: Clock) -> Vec<Section> {
    reading.hover_card(clock)
}

#[cfg(test)]
pub(crate) fn named_test_row(name: &'static str) -> ReadingRow<'static> {
    ReadingRow::glyph_only(
        name,
        IconGlyph::default(),
        ReadingDisplayState::Idle,
        Box::new(|_| Vec::new()),
    )
}
