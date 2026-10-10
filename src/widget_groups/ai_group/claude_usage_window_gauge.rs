use chrono::{DateTime, Utc};
use uom::si::{f64::Ratio, ratio::percent};

use crate::{
    data::{
        severity::Severity,
        source::ai::claude::{ClaudeUsageWindows, ExtraStatus, ExtraUsage, Pace, WindowUsage},
    },
    widget_groups::ai_group::{
        amount_text::{countdown, percent_text},
        spend_to_level::{level_from_pace, level_from_used},
    },
    widgets::{
        bar_widget::{BarWidget, Glyph, Occasional},
        simple_icons::SimpleIcon,
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for ClaudeUsageWindows {
    fn icon(&self) -> Glyph {
        SimpleIcon::Claude.into()
    }

    fn state(&self) -> WidgetState {
        let now = Utc::now();
        [self.session_pace(now), self.weekly_pace(now)]
            .into_iter()
            .flatten()
            .map(level_from_pace)
            .fold(level_from_used(self.session_used()), Severity::max)
            .into()
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(Ratio::new::<percent>(self.session_used()))
    }

    fn tooltip(&self) -> TooltipBody {
        let now = Utc::now();
        TooltipBody::sections(&[
            Section::untitled_dictionary([KeyValue::new("Plan", self.plan.as_str())]),
            window_section("Session", Some(&self.session), self.session_pace(now), now),
            window_section("Weekly", Some(&self.weekly), self.weekly_pace(now), now),
            window_section(
                "Weekly Sonnet",
                self.sonnet.as_ref(),
                self.sonnet_pace(now),
                now,
            ),
            extra_section(self.extra.as_ref()),
        ])
    }
}

impl Occasional for ClaudeUsageWindows {
    fn present(&self) -> bool {
        self.session_used() > 0.0
    }
}

fn window_section(
    title: &str,
    window: Option<&WindowUsage>,
    pace: Option<Pace>,
    now: DateTime<Utc>,
) -> Section {
    let pace_entry = pace.map(|pace| match pace {
        Pace::Lasts { at_reset } => KeyValue::new("Projected At Reset", percent_text(at_reset)),
        Pace::RunsOut { at } => KeyValue::new("Runs Out In", countdown(at, now)),
    });
    let used = window.map(|window| KeyValue::new("Used", percent_text(window.utilization)));
    let resets = window
        .and_then(|window| window.resets_at)
        .map(|at| KeyValue::new("Resets In", countdown(at, now)));
    Section::dictionary(title, [used, pace_entry, resets].into_iter().flatten())
}

fn extra_section(extra: Option<&ExtraUsage>) -> Section {
    let entries = extra.into_iter().flat_map(|extra| {
        let status = match &extra.status {
            ExtraStatus::Enabled => "enabled".to_owned(),
            ExtraStatus::Disabled(reason) => reason.clone(),
        };
        [
            Some(KeyValue::new("Status", status)),
            extra
                .spent
                .as_ref()
                .map(|spent| KeyValue::new("Spent This Month", spent.to_string())),
            extra
                .monthly_limit
                .as_ref()
                .map(|limit| KeyValue::new("Monthly Limit", limit.to_string())),
            extra
                .used_percent
                .map(|used| KeyValue::new("Used", percent_text(used))),
        ]
        .into_iter()
        .flatten()
    });
    Section::dictionary("Extra Usage", entries)
}
