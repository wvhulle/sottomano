use std::borrow::Cow;

use crate::{
    clock::Clock,
    group_indicators::{
        indicator_drawing::{
            IdentityOrdinal, IndicatorFace, IndicatorIcon, IndicatorReadout, ReadoutBadge,
            ReadoutGauge, ReadoutLabel,
        },
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::Section,
    widgets::{
        icon_glyph::IconGlyph,
        ongoing_toast::{IndicatorAlerts, LevelNotice},
    },
};

pub type DescribeCard<'a> = Box<dyn Fn(Clock) -> Vec<Section> + 'a>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelVisibility {
    Shown,
    Hidden,
}

pub struct ReadingRow<'a> {
    pub face: IndicatorFace,
    pub readout: IndicatorReadout,
    pub alerts: IndicatorAlerts,
    pub panel_visibility: PanelVisibility,
    pub card: DescribeCard<'a>,
}

impl<'a> ReadingRow<'a> {
    #[must_use]
    pub fn glyph_only(
        name: impl Into<Cow<'static, str>>,
        glyph: IconGlyph,
        state: IndicatorState,
        card: DescribeCard<'a>,
    ) -> Self {
        Self {
            face: IndicatorFace {
                name: name.into(),
                icon: IndicatorIcon::IconGlyph { glyph, state },
                ordinal: IdentityOrdinal::Unnumbered,
            },
            readout: IndicatorReadout {
                label: ReadoutLabel::Unlabeled,
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            alerts: IndicatorAlerts::default(),
            panel_visibility: PanelVisibility::Shown,
            card,
        }
    }

    #[must_use]
    pub fn shown_state(&self) -> IndicatorState {
        self.face.icon.display_state()
    }

    #[must_use]
    pub fn hover_card(&self, clock: Clock) -> Vec<Section> {
        (self.card)(clock)
    }

    #[must_use]
    pub fn in_state(self, state: IndicatorState) -> Self {
        let icon = match self.face.icon {
            IndicatorIcon::IconGlyph { glyph, .. } => IndicatorIcon::IconGlyph { glyph, state },
            unchanged_icon @ (IndicatorIcon::Image(_) | IndicatorIcon::Absent) => unchanged_icon,
        };
        let level = match self.alerts.level {
            LevelNotice::Announced { level, glyph } => LevelNotice::Announced {
                level,
                glyph: glyph.with_weight(state.weight()),
            },
            LevelNotice::Unannounced => LevelNotice::Unannounced,
        };
        Self {
            face: IndicatorFace { icon, ..self.face },
            alerts: IndicatorAlerts {
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
        IndicatorState::Idle,
        Box::new(|_| Vec::new()),
    )
}
