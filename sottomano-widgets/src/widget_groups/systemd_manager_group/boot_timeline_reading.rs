use std::time::Duration;

use chrono::Locale;
use iced::alignment::Horizontal;
use sottomano_config::severity::Severity;
use sottomano_sources::systemd_units_journal::boot_timeline::{
    BootTimeline, DurationComparison, RankedActivations,
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_durations::duration_text,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Cell, ColumnHeading, KeyValue, Section, TableRecord, TooltipBody},
        widget_state::WidgetState,
    },
};

fn timeline_record(
    stage: &str,
    took: Duration,
    comparison: Option<&DurationComparison>,
    meaning: &str,
) -> TableRecord<4> {
    TableRecord::new(
        stage.to_owned(),
        [
            Some(Cell::duration(took)),
            comparison.map(|comparison| Cell::duration(comparison.previous)),
            comparison.map(|comparison| Quantity::signed_share(comparison.change).into()),
            Some(Cell::Hint(meaning.to_owned())),
        ],
    )
    .level(comparison.map_or(Severity::Normal, |comparison| comparison.severity))
}

impl BarWidget for BootTimeline {
    fn name(&self) -> &'static str {
        "Boot timeline"
    }

    fn icon(&self) -> Glyph {
        Phosphor::RocketLaunch(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, locale: Locale) -> Option<String> {
        Some(match &self.startup_comparison {
            Some(comparison) => {
                Quantity::signed_share(comparison.os_startup.change).localized_text(locale)
            }
            None => duration_text(
                Duration::from_secs(self.phases.os_startup().total().as_secs()),
                locale,
            ),
        })
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        self.startup_comparison
            .as_ref()
            .map(|comparison| comparison.os_startup.severity)
            .unwrap_or_default()
            .into()
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        let comparison = self.startup_comparison.as_ref();
        let login_gap = self
            .user_startup
            .as_ref()
            .map(|user| user.started_at.saturating_sub(self.system_startup.finish));
        let session = self.user_startup.as_ref().map(|user| user.duration);
        let timeline_records = self
            .phases
            .present()
            .into_iter()
            .map(|phase| {
                timeline_record(
                    phase.stage.name(),
                    phase.duration,
                    comparison.and_then(|comparison| comparison.stage(phase.stage)),
                    phase.stage.meaning(),
                )
            })
            .chain([timeline_record(
                "OS startup",
                self.phases.os_startup().total(),
                comparison.map(|comparison| &comparison.os_startup),
                "kernel, initrd and userspace together",
            )])
            .chain(login_gap.map(|gap| {
                timeline_record(
                    "login",
                    gap,
                    None,
                    "waiting for the first user session manager",
                )
            }))
            .chain(session.map(|duration| {
                timeline_record(
                    "user session",
                    duration,
                    None,
                    "user systemd starting units until the default target",
                )
            }));
        let caption = match comparison {
            Some(_) => {
                "Difference is this boot against the previous one in the journal. Firmware and \
                 loader are not recorded per boot. Amber and red crossed \
                 slower_boot_warning_percent and slower_boot_critical_percent and took at least a \
                 second longer."
            }
            None => "No previous boot in the journal to compare with.",
        };
        TooltipBody::sections([Section::column_table(
            [
                ColumnHeading::titled("Took"),
                ColumnHeading::titled("Previous"),
                ColumnHeading::titled("Difference"),
                ColumnHeading::titled("Meaning").align_x(Horizontal::Left),
            ],
            timeline_records,
        )
        .titled(("Boot stages", caption))])
    }
}

pub struct StartupUnits<'a> {
    pub timeline: &'a BootTimeline,
}

impl BarWidget for StartupUnits<'_> {
    fn name(&self) -> &'static str {
        "Startup units"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Stack(Weight::Regular).into()
    }

    fn badge(&self) -> Option<u32> {
        let count = self
            .timeline
            .system_startup
            .slowest_units
            .slow_count()
            .saturating_add(
                self.timeline
                    .user_startup
                    .as_ref()
                    .map_or(0, |user| user.slowest_units.slow_count()),
            );
        u32::try_from(count).ok().filter(|count| *count > 0)
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        let slowness_state = |ranked: &RankedActivations| -> WidgetState {
            ranked
                .slowest_first
                .iter()
                .map(|activation| activation.severity)
                .max()
                .unwrap_or_default()
                .into()
        };
        let user_state = self
            .timeline
            .user_startup
            .as_ref()
            .map(|user| slowness_state(&user.slowest_units));
        [
            Some(slowness_state(&self.timeline.system_startup.slowest_units)),
            user_state,
        ]
        .into_iter()
        .flatten()
        .map(WidgetState::severity)
        .max()
        .unwrap_or_default()
        .into()
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        let blame_section = |ranked: &RankedActivations, title: &str| {
            Section::dictionary(ranked.slowest_first.iter().map(|activation| {
                KeyValue::new(activation.name.clone(), Cell::duration(activation.duration))
                    .level(activation.severity)
            }))
            .titled((
                title,
                "Time from starting to active, for units up before startup finished. Devices and \
                 units under 0.1 s are left out; amber ones crossed slow_boot_unit_secs.",
            ))
        };
        let user_section = self
            .timeline
            .user_startup
            .as_ref()
            .map(|user| blame_section(&user.slowest_units, "Slowest user units at login"));
        let sections: Vec<Section> = [
            Some(blame_section(
                &self.timeline.system_startup.slowest_units,
                "Slowest system units at boot",
            )),
            user_section,
        ]
        .into_iter()
        .flatten()
        .collect();
        TooltipBody::sections(sections)
    }
}
