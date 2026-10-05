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
        tooltip_body::{Section, TooltipBody, row, section, untitled},
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
        let mut sections = vec![
            untitled(vec![row("Plan", self.plan.clone())]),
            window_section("Session", &self.session, self.session_pace(now), now),
            window_section("Weekly", &self.weekly, self.weekly_pace(now), now),
        ];
        sections.extend(
            self.sonnet
                .as_ref()
                .map(|sonnet| window_section("Weekly Sonnet", sonnet, self.sonnet_pace(now), now)),
        );
        sections.extend(self.extra.as_ref().map(extra_section));
        TooltipBody::Sections(sections)
    }
}

impl Occasional for ClaudeUsageWindows {
    fn present(&self) -> bool {
        self.session_used() > 0.0
    }
}

fn window_section(
    title: &str,
    window: &WindowUsage,
    pace: Option<Pace>,
    now: DateTime<Utc>,
) -> Section {
    let rows = [
        Some(row("Used", percent_text(window.utilization))),
        pace.map(|pace| match pace {
            Pace::Lasts { at_reset } => row("Projected At Reset", percent_text(at_reset)),
            Pace::RunsOut { at } => row("Runs Out In", countdown(at, now)),
        }),
        Some(row(
            "Resets In",
            window
                .resets_at
                .map_or_else(|| "—".to_owned(), |at| countdown(at, now)),
        )),
    ]
    .into_iter()
    .flatten()
    .collect();
    section(title, rows)
}

fn extra_section(extra: &ExtraUsage) -> Section {
    let status = match &extra.status {
        ExtraStatus::Enabled => "enabled".to_owned(),
        ExtraStatus::Disabled(reason) => reason.clone(),
    };
    let rows = [
        Some(row("Status", status)),
        extra
            .spent
            .as_ref()
            .map(|spent| row("Spent This Month", spent.to_string())),
        extra
            .monthly_limit
            .as_ref()
            .map(|limit| row("Monthly Limit", limit.to_string())),
        extra
            .used_percent
            .map(|used| row("Used", percent_text(used))),
    ]
    .into_iter()
    .flatten()
    .collect();
    section("Extra Usage", rows)
}
