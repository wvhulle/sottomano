use chrono::Locale;
use sottomano_toml_config::severity::Severity;
use uom::si::{f64::Ratio, ratio};

use super::{
    SeverityDirection, SeverityThresholds, SliderActivity, SliderControl, SliderGauge,
    SliderGlyphSource, SliderLabelFormat, SliderLevelAnnouncing, SliderSeverityScale,
};
use crate::{
    glyph_icons::icon_glyph::IconGlyph,
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{
            IdentityOrdinal, IndicatorFace, IndicatorIcon, IndicatorReadout, ReadoutBadge,
            ReadoutGauge, ReadoutLabel,
        },
        indicator_state::IndicatorState,
    },
    widgets::ongoing_toast::{IndicatorAlerts, LevelNotice},
};

impl SeverityThresholds {
    fn level(&self, value: f32) -> Severity {
        let hit = |threshold: f32| match self.severity_direction {
            SeverityDirection::HigherWorse => value >= threshold,
            SeverityDirection::LowerWorse => value <= threshold,
        };
        if hit(self.urgent) {
            Severity::Critical
        } else if hit(self.warn) {
            Severity::Warning
        } else {
            Severity::Normal
        }
    }
}

impl SliderControl {
    pub(super) fn severity(&self) -> Severity {
        match &self.severity_scale {
            SliderSeverityScale::Thresholded(thresholds) => {
                thresholds.level(self.expanding_slider.synced_value.shown_value())
            }
            SliderSeverityScale::Unrated => Severity::Normal,
        }
    }

    #[must_use]
    pub fn shown_state(&self) -> IndicatorState {
        match self.slider_activity {
            SliderActivity::Active => self.severity().into(),
            SliderActivity::Inactive => IndicatorState::Off,
        }
    }

    fn shown_label(&self, locale: Locale) -> ReadoutLabel {
        let SliderLabelFormat::Labeled(format) = self.label_format else {
            return ReadoutLabel::Unlabeled;
        };
        match format(self.expanding_slider.synced_value.shown_value(), locale) {
            Some(label) if !label.is_empty() => ReadoutLabel::Labeled(label),
            Some(_) | None => ReadoutLabel::Unlabeled,
        }
    }

    fn shown_gauge(&self) -> ReadoutGauge {
        if self.gauge == SliderGauge::Ungauged {
            return ReadoutGauge::Ungauged;
        }
        let fill = self.range_fill();
        ReadoutGauge::Gauged {
            fill,
            live: fill * Ratio::new::<ratio::ratio>(f64::from(self.live)),
        }
    }

    fn level_notice(&self, locale: Locale) -> LevelNotice {
        let SliderLevelAnnouncing::Announcing(announcement) = &self.level_announcing else {
            return LevelNotice::Unannounced;
        };
        match announcement.level(self.range_fill(), self.readout_text(locale)) {
            Some(level) => LevelNotice::Announced {
                level,
                glyph: self
                    .effective_icon()
                    .with_weight(self.shown_state().weight()),
            },
            None => LevelNotice::Unannounced,
        }
    }

    #[must_use]
    pub fn indicator(&self, locale: Locale) -> GroupIndicator<'_> {
        GroupIndicator::glyph_only(self.name.clone(), self.effective_icon(), self.shown_state())
            .with_readout(self.indicator_readout(locale))
            .with_alerts(self.indicator_alerts(locale))
            .with_hover_card(Box::new(|_| self.readout_hover_card()))
    }

    pub(crate) fn indicator_face(&self) -> IndicatorFace {
        IndicatorFace {
            name: self.name.clone(),
            icon: IndicatorIcon::IconGlyph {
                glyph: self.effective_icon(),
                state: self.shown_state(),
            },
            ordinal: IdentityOrdinal::Unnumbered,
        }
    }

    pub(crate) fn indicator_readout(&self, locale: Locale) -> IndicatorReadout {
        IndicatorReadout {
            label: self.shown_label(locale),
            gauge: self.shown_gauge(),
            badge: ReadoutBadge::Unbadged,
        }
    }

    pub(crate) fn indicator_alerts(&self, locale: Locale) -> IndicatorAlerts {
        IndicatorAlerts {
            level: self.level_notice(locale),
            ..IndicatorAlerts::default()
        }
    }

    pub(super) fn effective_icon(&self) -> IconGlyph {
        match self.glyph_source {
            SliderGlyphSource::ValueDriven(glyph) => glyph(
                self.expanding_slider.synced_value.shown_value(),
                self.slider_activity,
            )
            .into(),
            SliderGlyphSource::FixedIcon => self.icon,
        }
    }
}
