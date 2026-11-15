use std::borrow::Cow;

use crate::{
    clock::Clock,
    group_items::{
        reading_display_state::ReadingDisplayState,
        source_reading::{
            GroupItemIcon, IdentityOrdinal, ReadoutBadge, ReadoutGauge, ReadoutLabel, RowFace,
            RowReadout,
        },
    },
    hover_cards::hover_card_body::Section,
    widgets::{
        icon_glyph::IconGlyph,
        ongoing_toast::{LevelNotice, ReadingAlerts},
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

#[cfg(test)]
pub(crate) fn named_test_row(name: &'static str) -> ReadingRow<'static> {
    ReadingRow::glyph_only(
        name,
        IconGlyph::default(),
        ReadingDisplayState::Idle,
        Box::new(|_| Vec::new()),
    )
}
