use chrono::Utc;

use crate::{
    data::source::focus_ledger::{Accounting, Interval, ScreenRun, WakingWindow},
    widget_groups::ai_group::amount_text::duration_text,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::TimePart,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Cell, KeyValue, Section, TooltipBody},
    },
};

pub struct LongestRun<'a> {
    pub accounting: Accounting<'a>,
    pub waking: WakingWindow,
}

impl LongestRun<'_> {
    pub fn sections(&self) -> [Section; 2] {
        let now = Utc::now();
        let config = self.accounting.config;
        let longest = self
            .accounting
            .longest_run_within(self.waking.around(now), now);
        let today = Section::dictionary(
            [
                longest.map(|run| KeyValue::new("Longest run", duration_text(run.length()))),
                longest.map(|Interval { from, .. }| {
                    KeyValue::new(
                        "Started at",
                        Cell::Time {
                            at: from,
                            part: TimePart::TimeOfDay,
                        },
                    )
                }),
                longest.map(|Interval { until, .. }| {
                    KeyValue::new(
                        "Ended at",
                        Cell::Time {
                            at: until,
                            part: TimePart::TimeOfDay,
                        },
                    )
                }),
                self.accounting.screen_run(now).map(|ScreenRun { since }| {
                    KeyValue::new("Current run", duration_text(now - since))
                }),
            ]
            .into_iter()
            .flatten(),
        )
        .titled("Today");
        let thresholds = Section::dictionary([
            KeyValue::new(
                "Session warning",
                humantime::format_duration(config.session_warning).to_string(),
            ),
            KeyValue::new(
                "Session critical",
                humantime::format_duration(config.session_critical).to_string(),
            ),
        ])
        .titled("Thresholds");
        [today, thresholds]
    }
}

impl BarWidget for LongestRun<'_> {
    fn name(&self) -> &'static str {
        "Longest run"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Trophy(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        let now = Utc::now();
        self.accounting
            .longest_run_within(self.waking.around(now), now)
            .map(|run| duration_text(run.length()))
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(self.sections())
    }
}
