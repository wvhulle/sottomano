use chrono::{DateTime, Utc};
use uom::si::{f64::Ratio, ratio::percent};

use crate::{
    data::source::ai::claude::{ClaudeUsageWindows, ExtraStatus, ExtraUsage, WindowUsage},
    widget_groups::ai_group::{amount_text::percent_text, spend_to_level::level_from_used},
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
        level_from_used(self.session_used()).into()
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(Ratio::new::<percent>(self.session_used()))
    }

    fn tooltip(&self) -> TooltipBody {
        let now = Utc::now();
        let mut sections = vec![
            untitled(vec![row("Plan", self.plan.clone())]),
            window_section("Session", &self.session, now),
            window_section("Weekly", &self.weekly, now),
        ];
        sections.extend(
            self.sonnet
                .as_ref()
                .map(|sonnet| window_section("Weekly Sonnet", sonnet, now)),
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

fn window_section(title: &str, window: &WindowUsage, now: DateTime<Utc>) -> Section {
    section(
        title,
        vec![
            row("Used", percent_text(window.utilization)),
            row(
                "Resets In",
                window
                    .resets_at
                    .map_or_else(|| "—".to_owned(), |at| countdown(at, now)),
            ),
        ],
    )
}

fn countdown(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let remaining = at.signed_duration_since(now).max(chrono::Duration::zero());
    let hours = remaining.num_hours();
    let minutes = remaining.num_minutes() - hours * 60;
    format!("{hours}h {minutes}m")
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
            .map(|spent| row("Spent", spent.to_string())),
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
