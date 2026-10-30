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
        bar_widget::{BarWidget, Glyph},
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        spend_to_level::level_from_used,
        tooltip_body::{Section, TooltipBody},
        value_cells::{LabeledValue, ValueCell},
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
            .unwrap_or_else(|| level_from_used(self.fullest_window_used()))
            .into()
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(self.fullest_window_used())
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
                Section::dictionary(
                    self.plan
                        .as_deref()
                        .map(|plan| LabeledValue::new("Plan", plan)),
                ),
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
        self.fullest_window_used().get::<percent>() > 0.0
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
    let used = LabeledValue::new("Used", Quantity::share(window.utilization).decimals(2));
    let pace_entry = quota.and_then(Quota::pace).map(|pace| match pace {
        Pace::Lasts { at_reset } => {
            LabeledValue::new("Projected At Reset", Quantity::share(at_reset).decimals(2))
        }
        Pace::RunsOut { at } => LabeledValue::new("Runs Out In", ValueCell::duration(at - now)),
        Pace::Exceeded => LabeledValue::new("Used Up", "already"),
    });
    let resets = window
        .resets_at
        .map(|at| LabeledValue::new("Resets In", ValueCell::duration(at - now)));
    Section::dictionary(once(used).chain(pace_entry).chain(resets)).titled(title)
}

fn extra_section(extra: Option<&ExtraUsage>) -> Section {
    let entries = extra.into_iter().flat_map(|extra| {
        let (status, reason) = match &extra.status {
            ExtraStatus::Enabled => ("enabled", None),
            ExtraStatus::Disabled { reason } => ("disabled", reason.as_deref()),
        };
        [
            Some(LabeledValue::new("Status", status)),
            reason.map(|reason| LabeledValue::new("Reason", reason)),
            extra
                .spent
                .map(|spent| LabeledValue::new("Spent This Month", Quantity::spend(spent))),
            extra
                .monthly_limit
                .map(|limit| LabeledValue::new("Monthly Limit", Quantity::spend(limit))),
            extra
                .used_share
                .map(|used| LabeledValue::new("Used", Quantity::share(used).decimals(2))),
        ]
        .into_iter()
        .flatten()
    });
    Section::dictionary(entries).titled("Extra Usage")
}
