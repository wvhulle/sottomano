use std::{iter::once, time::Duration};

use chrono::Locale;
use iced::alignment::Horizontal;
use sottomano_desktop_sources::{
    SnapshotSource,
    systemd_units_journal::boot_timeline::{BootTimeline, DurationComparison, RankedActivations},
};
use sottomano_toml_config::severity::Severity;

use crate::{
    clock::Clock,
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table, headed_table},
    widgets::{
        keyed_record_table::{ColumnHeading, RecordCells, TableRecord},
        localized_durations::duration_text,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
};

fn timeline_record(
    stage: &str,
    took: Duration,
    comparison: Option<&DurationComparison>,
    meaning: &str,
) -> TableRecord<4> {
    TableRecord::with_cells(
        stage.to_owned(),
        [
            ValueCell::duration(took),
            comparison.map_or(ValueCell::Blank, |comparison| {
                ValueCell::duration(comparison.previous)
            }),
            comparison.map_or(ValueCell::Blank, |comparison| {
                Quantity::signed_share(comparison.change).into()
            }),
            ValueCell::Hint(meaning.to_owned()),
        ],
    )
    .level(comparison.map_or(Severity::Normal, |comparison| comparison.severity))
}

fn login_record(stage: &str, took: Duration, meaning: &str) -> TableRecord<2> {
    TableRecord::with_cells(
        stage.to_owned(),
        [
            ValueCell::duration(took),
            ValueCell::Hint(meaning.to_owned()),
        ],
    )
}

pub(super) fn boot_timeline_hover_card(timeline: &BootTimeline, _clock: Clock) -> Vec<Section> {
    let comparison = timeline.startup_comparison.as_ref();
    let login_gap = timeline
        .user_startup
        .as_ref()
        .and_then(|user| user.started_at.checked_sub(timeline.system_startup.finish));
    let session = timeline.user_startup.as_ref().map(|user| user.duration);
    let stage_records = timeline.phases.present().into_iter().map(|phase| {
        timeline_record(
            phase.stage.name(),
            phase.duration,
            comparison.and_then(|comparison| comparison.stage(phase.stage)),
            phase.stage.meaning(),
        )
    });
    let os_startup_total = timeline_record(
        "OS startup",
        timeline.phases.os_startup().total(),
        comparison.map(|comparison| &comparison.os_startup),
        "kernel, initrd and userspace together, shown on the bar",
    );
    let login_records: Vec<TableRecord<2>> = [
        login_gap
            .map(|gap| login_record("login", gap, "waiting for the first user session manager")),
        session.map(|duration| {
            login_record(
                "user session",
                duration,
                "user systemd starting units until the default target",
            )
        }),
    ]
    .into_iter()
    .flatten()
    .collect();
    let caption = match comparison {
        Some(_) => {
            "Difference is this boot against the previous one in the journal. Firmware and loader \
             are not recorded per boot. Amber and red crossed slower_boot_warning_percent and \
             slower_boot_critical_percent and took at least a second longer."
        }
        None => "No previous boot in the journal to compare with.",
    };
    let stages_section = Section::new(
        SectionTitle::captioned("Boot stages", caption),
        headed_table(
            [
                ColumnHeading::titled("Took"),
                ColumnHeading::titled("Previous"),
                ColumnHeading::titled("Difference"),
                ColumnHeading::titled("Meaning").align_x(Horizontal::Left),
            ],
            stage_records,
        )
        .with_total_record(RecordCells::from(os_startup_total)),
    );
    let login_section = (!login_records.is_empty()).then(|| {
        Section::new(
            "After boot",
            headed_table(
                [
                    ColumnHeading::titled("Took"),
                    ColumnHeading::titled("Meaning").align_x(Horizontal::Left),
                ],
                login_records,
            ),
        )
    });
    once(stages_section)
        .chain(login_section)
        .collect::<Vec<_>>()
}

pub(super) fn boot_timeline_reading_row(
    timeline: &BootTimeline,
    _clock: Clock,
    locale: Locale,
) -> ReadingRow<'_> {
    let label = match &timeline.startup_comparison {
        Some(comparison) => {
            Quantity::signed_share(comparison.os_startup.change).localized_text(locale)
        }
        None => duration_text(
            Duration::from_secs(timeline.phases.os_startup().total().as_secs()),
            locale,
        ),
    };
    let base = ReadingRow::glyph_only(
        BootTimeline::NAME,
        Phosphor::RocketLaunch(Weight::Regular).into(),
        timeline
            .startup_comparison
            .as_ref()
            .map(|comparison| comparison.os_startup.severity)
            .unwrap_or_default()
            .into(),
        Box::new(move |card_clock| boot_timeline_hover_card(timeline, card_clock)),
    );
    ReadingRow {
        readout: RowReadout {
            label: ReadoutLabel::Labeled(label),
            ..base.readout
        },
        ..base
    }
}

pub struct StartupUnits<'a> {
    pub timeline: &'a BootTimeline,
}

impl<'a> StartupUnits<'a> {
    fn slow_unit_count(&self) -> usize {
        self.timeline
            .system_startup
            .slowest_units
            .slow_count()
            .checked_add(
                self.timeline
                    .user_startup
                    .as_ref()
                    .map_or(0, |user| user.slowest_units.slow_count()),
            )
            .expect("fewer than usize::MAX units are slow to start")
    }

    fn state(&self) -> ReadingDisplayState {
        let slowness_state = |ranked: &RankedActivations| -> ReadingDisplayState {
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
        .map(ReadingDisplayState::severity)
        .max()
        .unwrap_or_default()
        .into()
    }

    pub fn hover_card(&self, _clock: Clock) -> Vec<Section> {
        let blame_section = |ranked: &RankedActivations, title: &str| {
            Section::new(
                SectionTitle::captioned(
                    title,
                    "Time from starting to active, for units up before startup finished. Devices \
                     and units under 0.1 s are left out; amber ones crossed slow_boot_unit_secs.",
                ),
                dictionary_table(ranked.slowest_first.iter().map(|activation| {
                    LabeledValue::new(
                        activation.name.clone(),
                        ValueCell::duration(activation.duration),
                    )
                    .level(activation.severity)
                })),
            )
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
        sections.into_iter().collect::<Vec<_>>()
    }

    pub fn reading_row(self, _clock: Clock, _locale: Locale) -> ReadingRow<'a> {
        let badge = match self.slow_unit_count() {
            0 => ReadoutBadge::Unbadged,
            count => ReadoutBadge::Counted(count),
        };
        let base = ReadingRow::glyph_only(
            "Startup units",
            Phosphor::Stack(Weight::Regular).into(),
            self.state(),
            Box::new(move |card_clock| self.hover_card(card_clock)),
        );
        ReadingRow {
            readout: RowReadout {
                badge,
                ..base.readout
            },
            ..base
        }
    }
}
