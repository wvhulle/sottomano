use std::num::NonZeroUsize;

use chrono::{DateTime, Local, Utc};
use iced::alignment::Horizontal;
use sottomano_desktop_sources::login_session_presence::focus_time_ledger::{
    Accounting, LAST_DAY, TakenBreak,
};

use crate::{
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table, headed_table},
    widgets::{
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_time::LocalizedTimeFormat,
        value_cells::{LabeledValue, ValueCell},
    },
};

const LISTED_BREAKS_LIMIT: NonZeroUsize = NonZeroUsize::new(12).unwrap();

const BREAK_TABLE_HEADINGS: [ColumnHeading; 3] = [
    ColumnHeading::titled("Ended"),
    ColumnHeading::titled("Lasted"),
    ColumnHeading::titled("Counted because").align_x(Horizontal::Left),
];

pub struct BreakLog<'a> {
    pub accounting: Accounting<'a>,
}

struct RecentBreaks {
    taken_breaks: Vec<TakenBreak>,
    now: DateTime<Utc>,
}

impl RecentBreaks {
    fn moment_cell(&self, at: DateTime<Utc>) -> ValueCell {
        let same_local_day =
            at.with_timezone(&Local).date_naive() == self.now.with_timezone(&Local).date_naive();
        ValueCell::Time {
            at,
            format: if same_local_day {
                LocalizedTimeFormat::HourAndMinute
            } else {
                LocalizedTimeFormat::AbbreviatedWeekdayAndMinute
            },
        }
    }

    fn break_record(&self, taken: &TakenBreak) -> TableRecord<3> {
        let ended = if taken.interval.until >= self.now {
            ValueCell::Hint("ongoing".to_owned())
        } else {
            self.moment_cell(taken.interval.until)
        };
        TableRecord::with_cells(
            self.moment_cell(taken.interval.from),
            [
                ended,
                ValueCell::duration(taken.interval.length()),
                ValueCell::Hint(taken.cause.label().to_owned()),
            ],
        )
    }

    fn breaks_table(&self) -> Option<Section> {
        (!self.taken_breaks.is_empty()).then(|| {
            Section::new(
                SectionTitle::captioned(
                    "Breaks in the last 24 hours".to_owned(),
                    "Most recent first.".to_owned(),
                ),
                headed_table(
                    BREAK_TABLE_HEADINGS,
                    self.taken_breaks
                        .iter()
                        .rev()
                        .map(|taken| self.break_record(taken)),
                )
                .truncated_to(LISTED_BREAKS_LIMIT),
            )
        })
    }
}

impl BreakLog<'_> {
    fn recent_breaks(&self, now: DateTime<Utc>) -> RecentBreaks {
        RecentBreaks {
            taken_breaks: self
                .accounting
                .taken_breaks_within(LAST_DAY.ending_at(now), now),
            now,
        }
    }

    fn break_rules(&self) -> Section {
        let config = self.accounting.config;
        Section::new(
            SectionTitle::captioned(
                "How breaks count".to_owned(),
                "Turning the screen off or suspending always counts as a break, however short."
                    .to_owned(),
            ),
            dictionary_table([
                LabeledValue::new(
                    "Idle after no input for",
                    ValueCell::duration(config.idle_detection_delay),
                ),
                LabeledValue::new(
                    "Idle pause becomes a break after",
                    ValueCell::duration(config.idle_after),
                ),
            ]),
        )
    }

    pub fn sections(&self, now: DateTime<Utc>) -> Vec<Section> {
        [
            Some(self.break_rules()),
            self.recent_breaks(now).breaks_table(),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl<'a> BreakLog<'a> {
    pub fn indicator(self, clock: Clock) -> GroupIndicator<'a> {
        GroupIndicator::glyph_only(
            "Breaks",
            Phosphor::Coffee(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(
                self.recent_breaks(clock.utc())
                    .taken_breaks
                    .len()
                    .to_string(),
            ),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |card_clock| self.hover_card(card_clock)))
    }

    pub fn hover_card(&self, clock: Clock) -> Vec<Section> {
        self.sections(clock.utc()).into_iter().collect::<Vec<_>>()
    }
}
