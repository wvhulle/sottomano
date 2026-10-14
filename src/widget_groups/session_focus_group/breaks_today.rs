use chrono::Utc;

use crate::{
    data::source::focus_ledger::{Accounting, Interval, WakingWindow},
    widget_groups::ai_group::amount_text::duration_text,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::TimePart,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Cell, KeyValue, Section, TooltipBody},
    },
};

pub struct BreaksToday<'a> {
    pub accounting: Accounting<'a>,
    pub waking: WakingWindow,
}

impl BreaksToday<'_> {
    pub fn sections(&self) -> [Section; 2] {
        let now = Utc::now();
        let breaks = self.accounting.breaks_within(self.waking.around(now), now);
        let longest = breaks.iter().copied().max_by_key(|gap| gap.length());
        let last = self.accounting.last_break(now);
        let today = Section::dictionary(
            [
                Some(KeyValue::new("Breaks", breaks.len().to_string())),
                longest.map(|gap| KeyValue::new("Longest", duration_text(gap.length()))),
                last.map(|Interval { from, .. }| {
                    KeyValue::new(
                        "Last break at",
                        Cell::Time {
                            at: from,
                            part: TimePart::TimeOfDay,
                        },
                    )
                }),
                last.map(|gap| KeyValue::new("Last break lasted", duration_text(gap.length()))),
            ]
            .into_iter()
            .flatten(),
        )
        .titled("Today");
        let thresholds = Section::dictionary([KeyValue::new(
            "Break after idling",
            humantime::format_duration(self.accounting.config.idle_after).to_string(),
        )])
        .titled("Thresholds");
        [today, thresholds]
    }
}

impl BarWidget for BreaksToday<'_> {
    fn name(&self) -> &'static str {
        "Breaks"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Coffee(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        let now = Utc::now();
        self.accounting
            .last_break(now)
            .filter(|gap| gap.until < now)
            .map(|gap| format!("{} ago", duration_text(now - gap.until)))
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(self.sections())
    }
}
