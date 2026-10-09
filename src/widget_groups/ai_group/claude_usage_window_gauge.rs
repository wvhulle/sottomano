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
        tooltip_body::{Section, TooltipBody},
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
            Section::untitled(&[("Plan", &self.plan)]),
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
    Section::titled(
        title,
        &[
            (
                "Used",
                window.map(|window| percent_text(window.utilization)),
            ),
            (
                "Projected At Reset",
                pace.and_then(|pace| match pace {
                    Pace::Lasts { at_reset } => Some(percent_text(at_reset)),
                    Pace::RunsOut { .. } => None,
                }),
            ),
            (
                "Runs Out In",
                pace.and_then(|pace| match pace {
                    Pace::RunsOut { at } => Some(countdown(at, now)),
                    Pace::Lasts { .. } => None,
                }),
            ),
            (
                "Resets In",
                window.map(|window| {
                    window
                        .resets_at
                        .map_or_else(|| "—".to_owned(), |at| countdown(at, now))
                }),
            ),
        ],
    )
}

fn extra_section(extra: Option<&ExtraUsage>) -> Section {
    Section::titled(
        "Extra Usage",
        &[
            (
                "Status",
                extra.map(|extra| match &extra.status {
                    ExtraStatus::Enabled => "enabled".to_owned(),
                    ExtraStatus::Disabled(reason) => reason.clone(),
                }),
            ),
            (
                "Spent This Month",
                extra.and_then(|extra| extra.spent.as_ref().map(ToString::to_string)),
            ),
            (
                "Monthly Limit",
                extra.and_then(|extra| extra.monthly_limit.as_ref().map(ToString::to_string)),
            ),
            (
                "Used",
                extra.and_then(|extra| extra.used_percent.map(percent_text)),
            ),
        ],
    )
}
