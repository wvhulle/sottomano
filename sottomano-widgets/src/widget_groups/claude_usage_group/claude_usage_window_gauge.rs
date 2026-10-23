use std::iter::once;

use chrono::{DateTime, Utc};
use sottomano_sources::ai_usage_spend::{
    claude_usage::{ClaudeUsageWindows, ExtraStatus, ExtraUsage, WindowUsage},
    usage_quota::{Pace, Quota},
};
use uom::si::{f64::Ratio, ratio::percent};

use crate::{
    clock::Clock,
    widgets::{
        amount_text::{countdown, percent_text},
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        spend_to_level::level_from_used,
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for ClaudeUsageWindows {
    fn name(&self) -> &'static str {
        "Claude usage"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Gauge(Weight::Regular).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::Gauge(Weight::Regular).into()
    }

    fn state(&self, clock: Clock) -> WidgetState {
        Quota::worst_severity(&self.quotas(clock.utc()))
            .unwrap_or_else(|| level_from_used(self.session_used()))
            .into()
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(self.session_used())
    }

    fn tooltip(&self, clock: Clock) -> TooltipBody {
        let now = clock.utc();
        let sonnet = self.sonnet.as_ref().map(|sonnet| {
            window_section(
                "Weekly Sonnet",
                sonnet,
                self.sonnet_quota(now).as_ref(),
                now,
            )
        });
        TooltipBody::sections(
            [
                Section::dictionary(self.plan.as_deref().map(|plan| KeyValue::new("Plan", plan))),
                window_section(
                    "Session",
                    &self.session,
                    self.session_quota(now).as_ref(),
                    now,
                ),
                window_section("Weekly", &self.weekly, self.weekly_quota(now).as_ref(), now),
            ]
            .into_iter()
            .chain(sonnet)
            .chain([extra_section(self.extra.as_ref())]),
        )
    }

    fn visible_in_bar(&self) -> bool {
        self.session_used().get::<percent>() > 0.0
    }

    fn quota_windows(&self, now: DateTime<Utc>) -> Vec<Quota> {
        self.quotas(now)
    }
}

fn window_section(
    title: &str,
    window: &WindowUsage,
    quota: Option<&Quota>,
    now: DateTime<Utc>,
) -> Section {
    let used = KeyValue::new("Used", percent_text(window.utilization));
    let pace_entry = quota.and_then(Quota::pace).map(|pace| match pace {
        Pace::Lasts { at_reset } => KeyValue::new("Projected At Reset", percent_text(at_reset)),
        Pace::RunsOut { at } => KeyValue::new("Runs Out In", countdown(at, now)),
        Pace::Exceeded => KeyValue::new("Used Up", "already"),
    });
    let resets = window
        .resets_at
        .map(|at| KeyValue::new("Resets In", countdown(at, now)));
    Section::dictionary(once(used).chain(pace_entry).chain(resets)).titled(title)
}

fn extra_section(extra: Option<&ExtraUsage>) -> Section {
    let entries = extra.into_iter().flat_map(|extra| {
        let (status, reason) = match &extra.status {
            ExtraStatus::Enabled => ("enabled", None),
            ExtraStatus::Disabled { reason } => ("disabled", reason.as_deref()),
        };
        [
            Some(KeyValue::new("Status", status)),
            reason.map(|reason| KeyValue::new("Reason", reason)),
            extra
                .spent
                .as_ref()
                .map(|spent| KeyValue::new("Spent This Month", spent.to_string())),
            extra
                .monthly_limit
                .as_ref()
                .map(|limit| KeyValue::new("Monthly Limit", limit.to_string())),
            extra
                .used_share
                .map(|used| KeyValue::new("Used", percent_text(used))),
        ]
        .into_iter()
        .flatten()
    });
    Section::dictionary(entries).titled("Extra Usage")
}
