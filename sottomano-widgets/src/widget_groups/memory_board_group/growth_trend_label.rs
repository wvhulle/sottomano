use std::time::Duration;

use chrono::Locale;
use quick_xml::escape::escape;
use sottomano_desktop_sources::cpu_memory_compute::memory_growth_trend::{
    GrowingMemoryProcess, MemoryGrowth,
};
use sottomano_toml_config::severity::Severity;

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::{GroupItemReading, OngoingToastThreshold},
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widgets::{
        icon_glyph::IconGlyph,
        localized_durations::duration_text,
        localized_quantities::Quantity,
        ongoing_toast::definition_list,
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
};

fn growth_rate_text(growth_per_minute: bytesize::ByteSize, locale: Locale) -> String {
    format!(
        "+{}/min",
        Quantity::size(growth_per_minute).localized_text(locale)
    )
}

fn near_exhaustion(growth: &MemoryGrowth) -> Option<Duration> {
    growth
        .exhaustion_projection
        .filter(|_| growth.filling_severity > Severity::Normal)
}

fn growing_process_row(process: &GrowingMemoryProcess, locale: Locale) -> (String, String) {
    (
        escape(process.name.as_str()).into_owned(),
        growth_rate_text(process.growth_per_minute, locale),
    )
}

impl GroupItemReading for MemoryGrowth {
    fn name(&self) -> &'static str {
        "RAM growth"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::TrendUp(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, locale: Locale) -> Option<String> {
        Some(self.used_per_minute.map_or_else(
            || "steady".to_owned(),
            |growth_per_minute| growth_rate_text(growth_per_minute, locale),
        ))
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        self.filling_severity.into()
    }

    fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
        OngoingToastThreshold::NotifyFromSeverity {
            minimum_severity: Severity::Warning,
            summary_text: "RAM filling fast",
        }
    }

    fn ongoing_toast_body(&self, _clock: Clock, locale: Locale) -> Option<String> {
        let exhaustion_row = near_exhaustion(self)
            .map(|remaining| ("Full in".to_owned(), duration_text(remaining, locale)));
        let body_rows: Vec<(String, String)> = exhaustion_row
            .into_iter()
            .chain(
                self.growing_processes
                    .iter()
                    .map(|process| growing_process_row(process, locale)),
            )
            .collect();
        (!body_rows.is_empty()).then(|| {
            definition_list(
                body_rows
                    .iter()
                    .map(|(row_key, row_value)| (row_key.as_str(), row_value.clone())),
            )
        })
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let trend_rows = [LabeledValue::new(
            "Growth per minute",
            self.used_per_minute.map_or_else(
                || ValueCell::from("steady".to_owned()),
                |growth_per_minute| Quantity::size(growth_per_minute).into(),
            ),
        )]
        .into_iter()
        .chain(
            near_exhaustion(self)
                .map(|remaining| LabeledValue::new("Full in", ValueCell::duration(remaining))),
        );
        let process_rows = self.growing_processes.iter().map(|process| {
            LabeledValue::new(
                process.name.clone(),
                Quantity::size(process.growth_per_minute),
            )
        });
        HoverCardBody::sections(
            [Section::dictionary(trend_rows)].into_iter().chain(
                (!self.growing_processes.is_empty()).then(|| {
                    Section::dictionary(process_rows).titled("Growing fastest, per minute")
                }),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use bytesize::ByteSize;

    use super::*;
    use crate::widgets::ongoing_toast::reading_display_state_alert_severity;

    fn growth_towards(
        exhaustion_projection: Option<Duration>,
        filling_severity: Severity,
    ) -> MemoryGrowth {
        MemoryGrowth {
            used_per_minute: exhaustion_projection.map(|_| ByteSize::gib(2)),
            exhaustion_projection,
            filling_severity,
            growing_processes: vec![GrowingMemoryProcess {
                pid: 4,
                name: "firefox".to_owned(),
                resident: ByteSize::gib(6),
                growth_per_minute: ByteSize::gib(2),
            }],
        }
    }

    #[test]
    fn a_steady_ram_is_quiet() {
        let steady = growth_towards(None, Severity::Normal);
        assert_eq!(steady.state(Clock::real()), ReadingDisplayState::Idle);
        assert_eq!(
            steady.label(Clock::real(), Locale::en_US).as_deref(),
            Some("steady")
        );
    }

    #[test]
    fn a_near_projection_toasts_from_warning() {
        assert_eq!(
            reading_display_state_alert_severity(
                &growth_towards(Some(Duration::from_secs(200)), Severity::Warning),
                Clock::real()
            ),
            Some(Severity::Warning)
        );
        assert_eq!(
            growth_towards(Some(Duration::from_secs(30)), Severity::Critical).state(Clock::real()),
            ReadingDisplayState::Critical
        );
        assert_eq!(
            growth_towards(Some(Duration::from_secs(3_600)), Severity::Normal).state(Clock::real()),
            ReadingDisplayState::Idle
        );
    }

    #[test]
    fn toast_body_lists_the_growing_processes_as_rows() {
        let body = growth_towards(Some(Duration::from_secs(40)), Severity::Critical)
            .ongoing_toast_body(Clock::real(), Locale::en_US)
            .expect("a growing reading has a body");
        assert!(body.starts_with("<dl><dt>Full in</dt><dd>"), "{body}");
        assert!(body.contains("<dt>firefox</dt><dd>+"), "{body}");
    }
}
