use chrono::{DateTime, Locale, Utc};
use iced::alignment::Horizontal;
use sottomano_sources::login_session_presence::focus_time_ledger::{
    Accounting, TakenBreak, WakingWindow,
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Cell, ColumnHeading, KeyValue, Section, TableRecord, TooltipBody},
    },
};

const LISTED_BREAKS_LIMIT: usize = 12;

const BREAK_TABLE_HEADINGS: [ColumnHeading; 3] = [
    ColumnHeading::new("Ended"),
    ColumnHeading::new("Lasted"),
    ColumnHeading::new("Counted because").align_x(Horizontal::Left),
];

pub struct BreakLog<'a> {
    pub accounting: Accounting<'a>,
    pub waking: WakingWindow,
}

struct TodaysBreaks {
    taken_breaks: Vec<TakenBreak>,
    now: DateTime<Utc>,
}

impl TodaysBreaks {
    fn break_record(&self, taken: &TakenBreak) -> TableRecord<3> {
        let ended = if taken.interval.until >= self.now {
            Cell::Hint("ongoing".to_owned())
        } else {
            Cell::Time {
                at: taken.interval.until,
                format: LocalizedTimeFormat::HourAndMinute,
            }
        };
        TableRecord::new(
            Cell::Time {
                at: taken.interval.from,
                format: LocalizedTimeFormat::HourAndMinute,
            },
            [
                Some(ended),
                Some(Cell::duration(taken.interval.length())),
                Some(Cell::Hint(taken.cause.label().to_owned())),
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
            .titled(("Breaks today".to_owned(), self.table_caption()))
        })
    }

    fn breaks_summary(&self) -> Section {
        let longest_break = self
            .taken_breaks
            .iter()
            .map(|taken| taken.interval.length())
            .max();
        Section::dictionary(
            [
                Some(KeyValue::new(
                    "Breaks taken",
                    self.taken_breaks.len().to_string(),
                )),
                longest_break.map(|length| KeyValue::new("Longest break", Cell::duration(length))),
            ]
            .into_iter()
            .flatten(),
        )
        .titled((
            "Today (waking hours)".to_owned(),
            "Waking hours run from the end of your quiet schedule to its start.".to_owned(),
        ))
    }
}

impl BreakLog<'_> {
    fn todays_breaks(&self, now: DateTime<Utc>) -> TodaysBreaks {
        TodaysBreaks {
            taken_breaks: self
                .accounting
                .taken_breaks_within(self.waking.around(now), now),
            now,
        }
    }

    fn break_rules(&self) -> Section {
        let config = self.accounting.config;
        Section::dictionary([
            KeyValue::new(
                "Idle after no input for",
                Cell::duration(config.idle_detection_delay),
            ),
            KeyValue::new(
                "Idle pause becomes a break after",
                Cell::duration(config.idle_after),
            ),
        ])
        .titled((
            "How breaks count".to_owned(),
            "Turning the screen off or suspending always counts as a break, however short."
                .to_owned(),
        ))
    }

    pub fn sections(&self, now: DateTime<Utc>) -> Vec<Section> {
        let todays_breaks = self.todays_breaks(now);
        [
            Some(self.break_rules()),
            todays_breaks.breaks_table(),
            Some(todays_breaks.breaks_summary()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl BarWidget for BreakLog<'_> {
    fn name(&self) -> &'static str {
        "Breaks"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Coffee(Weight::Regular).into()
    }

    fn label(&self, clock: Clock, _locale: Locale) -> Option<String> {
        Some(
            self.todays_breaks(clock.utc())
                .taken_breaks
                .len()
                .to_string(),
        )
    }

    fn tooltip(&self, clock: Clock) -> TooltipBody {
        TooltipBody::sections(self.sections(clock.utc()))
    }
}
