use chrono::{DateTime, Local, Locale, Utc};
use iced::alignment::Horizontal;
use sottomano_sources::login_session_presence::focus_time_ledger::{
    Accounting, LAST_DAY, TakenBreak,
};

use crate::{
    clock::Clock,
    widgets::{
        group_item_reading::GroupItemReading,
        hover_card_body::{HoverCardBody, Section},
        icon_glyph::IconGlyph,
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
};

const LISTED_BREAKS_LIMIT: usize = 12;

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
        TableRecord::new(
            self.moment_cell(taken.interval.from),
            [
                ended,
                ValueCell::duration(taken.interval.length()),
                ValueCell::Hint(taken.cause.label().to_owned()),
            ],
        )
    }

    fn listed_breaks(&self) -> &[TakenBreak] {
        &self.taken_breaks[self.taken_breaks.len().saturating_sub(LISTED_BREAKS_LIMIT)..]
    }

    fn table_caption(&self) -> String {
        let listed_count = self.listed_breaks().len();
        if listed_count < self.taken_breaks.len() {
            format!(
                "{listed_count} most recent of {}, by start time.",
                self.taken_breaks.len()
            )
        } else {
            "By start time.".to_owned()
        }
    }

    fn breaks_table(&self) -> Option<Section> {
        (!self.taken_breaks.is_empty()).then(|| {
            Section::column_table(
                BREAK_TABLE_HEADINGS,
                self.listed_breaks()
                    .iter()
                    .map(|taken| self.break_record(taken)),
            )
            .titled((
                "Breaks in the last 24 hours".to_owned(),
                self.table_caption(),
            ))
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
        Section::dictionary([
            LabeledValue::new(
                "Idle after no input for",
                ValueCell::duration(config.idle_detection_delay),
            ),
            LabeledValue::new(
                "Idle pause becomes a break after",
                ValueCell::duration(config.idle_after),
            ),
        ])
        .titled((
            "How breaks count".to_owned(),
            "Turning the screen off or suspending always counts as a break, however short."
                .to_owned(),
        ))
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

impl GroupItemReading for BreakLog<'_> {
    fn name(&self) -> &'static str {
        "Breaks"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Coffee(Weight::Regular).into()
    }

    fn label(&self, clock: Clock, _locale: Locale) -> Option<String> {
        Some(
            self.recent_breaks(clock.utc())
                .taken_breaks
                .len()
                .to_string(),
        )
    }

    fn hover_card(&self, clock: Clock) -> HoverCardBody {
        HoverCardBody::sections(self.sections(clock.utc()))
    }
}
