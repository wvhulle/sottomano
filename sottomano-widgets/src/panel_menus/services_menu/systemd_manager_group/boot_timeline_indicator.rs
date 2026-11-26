use std::{iter::once, time::Duration};

use chrono::Locale;
use iced::alignment::Horizontal;
use sottomano_desktop_sources::{
    SnapshotSource,
    systemd_units_journal::boot_timeline::{
        BootTimeline, DurationComparison, PreviousBootComparison, RankedActivations,
        UserStartupProgress,
    },
};
use sottomano_toml_config::severity::Severity;

use super::SystemdManagerGroup;
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{ReadoutBadge, ReadoutLabel},
        indicator_group_content::IndicatorListing,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table, headed_table},
    widgets::{
        keyed_record_table::{ColumnHeading, RecordCells, TableRecord},
        localized_durations::duration_text,
        localized_quantities::Quantity,
        value_cells::{LabeledValue, ValueCell},
    },
};

const BOOT_TIMELINE_GLYPH: Phosphor = Phosphor::RocketLaunch(Weight::Regular);

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

pub(super) fn boot_timeline_hover_card(timeline: &BootTimeline) -> Vec<Section> {
    let comparison = match &timeline.startup_comparison {
        PreviousBootComparison::Compared(comparison) => Some(comparison),
        PreviousBootComparison::NoPreviousBoot => None,
    };
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
        timeline.phases.os_startup.total(),
        comparison.map(|comparison| &comparison.os_startup),
        "kernel, initrd and userspace together, shown on the bar",
    );
    let mut login_records: Vec<TableRecord<2>> = Vec::new();
    if let UserStartupProgress::Finished(user) = &timeline.user_startup {
        if let Some(gap) = user.started_at.checked_sub(timeline.system_startup.finish) {
            login_records.push(login_record(
                "login",
                gap,
                "waiting for the first user session manager",
            ));
        }
        login_records.push(login_record(
            "user session",
            user.duration,
            "user systemd starting units until the default target",
        ));
    }
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

pub(super) fn boot_timeline_indicator(
    timeline: &BootTimeline,
    locale: Locale,
) -> GroupIndicator<'_> {
    let label = match &timeline.startup_comparison {
        PreviousBootComparison::Compared(comparison) => {
            Quantity::signed_share(comparison.os_startup.change).localized_text(locale)
        }
        PreviousBootComparison::NoPreviousBoot => duration_text(
            Duration::from_secs(timeline.phases.os_startup.total().as_secs()),
            locale,
        ),
    };
    let base = GroupIndicator::glyph_only(
        BootTimeline::NAME,
        Phosphor::RocketLaunch(Weight::Regular).into(),
        match &timeline.startup_comparison {
            PreviousBootComparison::Compared(comparison) => comparison.os_startup.severity,
            PreviousBootComparison::NoPreviousBoot => Severity::default(),
        }
        .into(),
    )
    .with_hover_card(Box::new(move |_| boot_timeline_hover_card(timeline)));
    base.with_label(ReadoutLabel::Labeled(label))
}

pub struct StartupUnits<'view> {
    pub timeline: &'view BootTimeline,
}

impl<'view> StartupUnits<'view> {
    fn slow_unit_count(&self) -> usize {
        self.timeline
            .system_startup
            .slowest_units
            .slow_count()
            .checked_add(match &self.timeline.user_startup {
                UserStartupProgress::Finished(user) => user.slowest_units.slow_count(),
                UserStartupProgress::AwaitingLogin => 0,
            })
            .expect("fewer than usize::MAX units are slow to start")
    }

    fn state(&self) -> IndicatorState {
        let slowest_severity = |ranked: &RankedActivations| -> Severity {
            ranked
                .slowest_first
                .iter()
                .map(|activation| activation.severity)
                .max()
                .unwrap_or_default()
        };
        let system_severity = slowest_severity(&self.timeline.system_startup.slowest_units);
        match &self.timeline.user_startup {
            UserStartupProgress::Finished(user) => {
                system_severity.max(slowest_severity(&user.slowest_units))
            }
            UserStartupProgress::AwaitingLogin => system_severity,
        }
        .into()
    }

    pub fn hover_card(&self) -> Vec<Section> {
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
        let mut sections = vec![blame_section(
            &self.timeline.system_startup.slowest_units,
            "Slowest system units at boot",
        )];
        if let UserStartupProgress::Finished(user) = &self.timeline.user_startup {
            sections.push(blame_section(
                &user.slowest_units,
                "Slowest user units at login",
            ));
        }
        sections
    }

    pub fn indicator(self) -> GroupIndicator<'view> {
        let badge = match self.slow_unit_count() {
            0 => ReadoutBadge::Unbadged,
            count => ReadoutBadge::Counted(count),
        };
        let base = GroupIndicator::glyph_only(
            "Startup units",
            Phosphor::Stack(Weight::Regular).into(),
            self.state(),
        )
        .with_hover_card(Box::new(move |_| self.hover_card()));
        base.with_badge(badge)
    }
}

impl SystemdManagerGroup {
    pub(super) fn boot_timeline_listing(&self, locale: Locale) -> IndicatorListing<'_> {
        self.boot.indicator_listing(
            |timeline| {
                vec![
                    boot_timeline_indicator(timeline, locale).with_hover_card(Box::new(
                        move |_| self.boot.sections_described_by(boot_timeline_hover_card),
                    )),
                    StartupUnits { timeline }
                        .indicator()
                        .with_hover_card(Box::new(move |_| {
                            self.boot
                                .hover_card_with(StartupUnits { timeline }.hover_card())
                        })),
                ]
            },
            BOOT_TIMELINE_GLYPH.into(),
        )
    }
}
