use std::time::Duration;

use chrono::Locale;
use corredo::changing_text::escape_markup;
use sottomano_desktop_sources::{
    SnapshotSource,
    cpu_memory_compute::memory_growth_trend::{
        ExhaustionProjection, GrowingMemoryProcess, MemoryGrowth, MemoryUsageTrend,
    },
};
use sottomano_toml_config::severity::Severity;

use super::{MemoryBoardGroup, MemoryBoardInteraction, MemoryBoardSource};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_group_content::IndicatorListing,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_durations::duration_text,
        localized_quantities::Quantity,
        ongoing_toast::{
            IndicatorAlerts, NotificationBody, NotificationProgress, StateToast, definition_list,
        },
        value_cells::{LabeledValue, ValueCell},
    },
};

const MEMORY_GROWTH_GLYPH: Phosphor = Phosphor::TrendUp(Weight::Regular);

fn growth_rate_text(growth_per_minute: bytesize::ByteSize, locale: Locale) -> String {
    format!(
        "+{}/min",
        Quantity::size(growth_per_minute).localized_text(locale)
    )
}

fn near_exhaustion(growth: &MemoryGrowth) -> Option<Duration> {
    match growth.trend {
        MemoryUsageTrend::Rising {
            exhaustion: ExhaustionProjection::FullWithin(remaining),
            ..
        } if growth.filling_severity > Severity::Normal => Some(remaining),
        MemoryUsageTrend::Steady | MemoryUsageTrend::Rising { .. } => None,
    }
}

fn growing_process_row(process: &GrowingMemoryProcess, locale: Locale) -> (String, String) {
    (
        escape_markup(process.name.as_str()).into_owned(),
        growth_rate_text(process.growth_per_minute, locale),
    )
}

pub fn memory_growth_indicator(growth: &MemoryGrowth, locale: Locale) -> GroupIndicator<'_> {
    let label = match growth.trend {
        MemoryUsageTrend::Steady => "steady".to_owned(),
        MemoryUsageTrend::Rising {
            used_per_minute, ..
        } => growth_rate_text(used_per_minute, locale),
    };
    let toast_body = growth_toast_body(growth, locale);
    GroupIndicator::glyph_only(
        MemoryGrowth::NAME,
        Phosphor::TrendUp(Weight::Regular).into(),
        growth.filling_severity.into(),
    )
    .with_readout(IndicatorReadout {
        label: ReadoutLabel::Labeled(label),
        gauge: ReadoutGauge::Ungauged,
        badge: ReadoutBadge::Unbadged,
    })
    .with_alerts(IndicatorAlerts {
        state_toast: StateToast::NotifyFromSeverity {
            minimum_severity: Severity::Warning,
            summary_text: "RAM filling fast",
            body: toast_body.map_or(NotificationBody::Bodiless, NotificationBody::Text),
            progress: NotificationProgress::Unmeasured,
        },
        ..IndicatorAlerts::default()
    })
    .with_hover_card(Box::new(move |_| memory_growth_hover_card(growth)))
}

fn growth_toast_body(growth: &MemoryGrowth, locale: Locale) -> Option<String> {
    let exhaustion_indicator = near_exhaustion(growth)
        .map(|remaining| ("Full in".to_owned(), duration_text(remaining, locale)));
    let body_rows: Vec<(String, String)> = exhaustion_indicator
        .into_iter()
        .chain(
            growth
                .growing_processes
                .iter()
                .map(|process| growing_process_row(process, locale)),
        )
        .collect();
    (!body_rows.is_empty()).then(|| {
        definition_list(body_rows.iter().map(|(indicator_key, indicator_value)| {
            (indicator_key.as_str(), indicator_value.clone())
        }))
    })
}

pub fn memory_growth_hover_card(growth: &MemoryGrowth) -> Vec<Section> {
    let trend_rows = [LabeledValue::new(
        "Growth per minute",
        match growth.trend {
            MemoryUsageTrend::Steady => ValueCell::from("steady".to_owned()),
            MemoryUsageTrend::Rising {
                used_per_minute, ..
            } => Quantity::size(used_per_minute).into(),
        },
    )]
    .into_iter()
    .chain(
        near_exhaustion(growth)
            .map(|remaining| LabeledValue::new("Full in", ValueCell::duration(remaining))),
    );
    let process_rows = growth.growing_processes.iter().map(|process| {
        LabeledValue::new(
            process.name.clone(),
            Quantity::size(process.growth_per_minute),
        )
    });
    [Section::new("Trend", dictionary_table(trend_rows))]
        .into_iter()
        .chain((!growth.growing_processes.is_empty()).then(|| {
            Section::new(
                "Growing fastest, per minute",
                dictionary_table(process_rows),
            )
        }))
        .collect()
}

impl MemoryBoardGroup {
    pub(super) fn memory_growth_listing(&self, locale: Locale) -> IndicatorListing<'_> {
        self.memory_growth
            .indicator_listing(
                |held_snapshot| memory_growth_indicator(held_snapshot, locale).when_shown(),
                MEMORY_GROWTH_GLYPH.into(),
            )
            .map_indicators(|row| self.wired_memory_growth_indicator(row))
    }

    fn wired_memory_growth_indicator<'view>(
        &'view self,
        growth_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        growth_indicator
            .with_hover_card(Box::new(move |_| {
                self.memory_growth
                    .sections_described_by(memory_growth_hover_card)
            }))
            .refreshed_by(MemoryBoardInteraction::Refresh(MemoryBoardSource::Growth))
    }
}

#[cfg(test)]
mod tests {
    use bytesize::ByteSize;

    use super::*;
    use crate::group_indicators::{
        group_indicator::indicator_toast_severity, indicator_state::IndicatorState,
    };

    fn growth_towards(trend: MemoryUsageTrend, filling_severity: Severity) -> MemoryGrowth {
        MemoryGrowth {
            trend,
            filling_severity,
            growing_processes: vec![GrowingMemoryProcess {
                pid: 4,
                name: "firefox".to_owned(),
                resident: ByteSize::gib(6),
                growth_per_minute: ByteSize::gib(2),
            }],
        }
    }

    fn full_within(remaining: Duration) -> MemoryUsageTrend {
        MemoryUsageTrend::Rising {
            used_per_minute: ByteSize::gib(2),
            exhaustion: ExhaustionProjection::FullWithin(remaining),
        }
    }

    #[test]
    fn a_steady_ram_is_quiet() {
        let steady = growth_towards(MemoryUsageTrend::Steady, Severity::Normal);
        let steady_indicator = memory_growth_indicator(&steady, Locale::en_US);
        assert_eq!(steady_indicator.shown_state(), IndicatorState::Idle);
        assert_eq!(
            steady_indicator
                .shown_readout()
                .expect("a readout indicator")
                .label,
            ReadoutLabel::Labeled("steady".to_owned())
        );
    }

    #[test]
    fn a_near_projection_toasts_from_warning() {
        assert_eq!(
            indicator_toast_severity(&memory_growth_indicator(
                &growth_towards(full_within(Duration::from_secs(200)), Severity::Warning),
                Locale::default(),
            )),
            Some(Severity::Warning)
        );
        assert_eq!(
            memory_growth_indicator(
                &growth_towards(full_within(Duration::from_secs(30)), Severity::Critical),
                Locale::default()
            )
            .shown_state(),
            IndicatorState::Critical
        );
        assert_eq!(
            memory_growth_indicator(
                &growth_towards(full_within(Duration::from_secs(3_600)), Severity::Normal),
                Locale::default()
            )
            .shown_state(),
            IndicatorState::Idle
        );
    }

    #[test]
    fn toast_body_lists_the_growing_processes_as_rows() {
        let body = growth_toast_body(
            &growth_towards(full_within(Duration::from_secs(40)), Severity::Critical),
            Locale::en_US,
        )
        .expect("a growing reading has a body");
        assert!(body.starts_with("<dl><dt>Full in</dt><dd>"), "{body}");
        assert!(body.contains("<dt>firefox</dt><dd>+"), "{body}");
    }
}
