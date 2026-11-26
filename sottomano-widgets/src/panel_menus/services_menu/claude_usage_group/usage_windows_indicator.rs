use std::iter::once;

use chrono::{DateTime, Utc};
use sottomano_desktop_sources::{
    SnapshotSource,
    ai_usage_spend::{
        claude_usage::{
            ClaudeUsageWindows, ExtraAllowance, ExtraSpend, ExtraUsageOffer, SonnetWindow,
            WindowUsage,
        },
        subscription_plan::SubscriptionPlan,
        usage_quota::{Pace, Quota},
    },
};
use uom::si::ratio::percent;

use super::{
    ClaudeUsageBusRequest, ClaudeUsageGroup, ClaudeUsageInteraction,
    enterprise_spend_indicator::extra_status_rows,
};
use crate::{
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_group_content::IndicatorListing,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::Quantity,
        ongoing_toast::IndicatorAlerts,
        spend_to_level::level_from_used,
        value_cells::{LabeledValue, ValueCell},
    },
};

pub fn claude_usage_windows_indicator(
    windows: &ClaudeUsageWindows,
    clock: Clock,
) -> GroupIndicator<'_> {
    let quota_windows = windows.quotas(clock.utc());
    let fullest_used = windows.fullest_window_used();
    let state = Quota::worst_severity(&quota_windows)
        .unwrap_or_else(|| level_from_used(fullest_used))
        .into();
    let panel_visibility = if fullest_used.get::<percent>() > 0.0 {
        PanelVisibility::Shown
    } else {
        PanelVisibility::Hidden
    };
    GroupIndicator::glyph_only(
        ClaudeUsageWindows::NAME,
        Phosphor::Gauge(Weight::Regular).into(),
        state,
    )
    .with_readout(IndicatorReadout {
        label: ReadoutLabel::Unlabeled,
        gauge: ReadoutGauge::Gauged {
            fill: fullest_used,
            live: fullest_used,
        },
        badge: ReadoutBadge::Unbadged,
    })
    .with_alerts(IndicatorAlerts {
        quota_windows,
        ..IndicatorAlerts::default()
    })
    .with_panel_visibility(panel_visibility)
    .with_hover_card(Box::new(move |card_clock| {
        claude_usage_windows_hover_card(windows, card_clock)
    }))
}

pub fn claude_usage_windows_hover_card(windows: &ClaudeUsageWindows, clock: Clock) -> Vec<Section> {
    let now = clock.utc();
    let mut subscription_rows = Vec::new();
    if let SubscriptionPlan::Named(plan) = &windows.plan {
        subscription_rows.push(LabeledValue::new("Plan", plan.as_str()));
    }
    let mut sections = vec![
        Section::new("Subscription", dictionary_table(subscription_rows)),
        window_section(
            "Session",
            &windows.session,
            windows.session_quota(now).as_ref(),
            now,
        ),
        window_section(
            "Weekly",
            &windows.weekly,
            windows.weekly_quota(now).as_ref(),
            now,
        ),
    ];
    if let SonnetWindow::Tracked(sonnet) = &windows.sonnet {
        sections.push(window_section(
            "Weekly Sonnet",
            sonnet,
            windows.sonnet_quota(now).as_ref(),
            now,
        ));
    }
    sections.push(extra_section(&windows.extra));
    sections
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
    Section::new(
        title,
        dictionary_table(once(used).chain(pace_entry).chain(resets)),
    )
}

fn extra_section(extra: &ExtraUsageOffer) -> Section {
    let ExtraUsageOffer::Taken(extra) = extra else {
        return Section::new("Extra Usage", dictionary_table([]));
    };
    let mut extra_rows = extra_status_rows(&extra.status);
    if let ExtraSpend::Spent(spent) = extra.spent {
        extra_rows.push(LabeledValue::new(
            "Spent This Month",
            Quantity::spend(spent),
        ));
    }
    if let ExtraAllowance::Capped {
        monthly_limit,
        used_share,
    } = extra.allowance
    {
        extra_rows.push(LabeledValue::new(
            "Monthly Limit",
            Quantity::spend(monthly_limit),
        ));
        extra_rows.push(LabeledValue::new(
            "Used",
            Quantity::share(used_share).decimals(2),
        ));
    }
    Section::new("Extra Usage", dictionary_table(extra_rows))
}

impl ClaudeUsageGroup {
    pub(super) fn usage_windows_listing(&self, clock: Clock) -> IndicatorListing<'_> {
        self.usage_windows
            .indicator_listing(
                |held_snapshot| vec![claude_usage_windows_indicator(held_snapshot, clock)],
                Phosphor::Gauge(Weight::Regular).into(),
            )
            .map_indicators(|row| self.usage_windows_indicator(row))
    }

    pub(super) fn usage_windows_indicator<'view>(
        &'view self,
        windows_row: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        windows_row
            .with_hover_card(Box::new(move |card_clock| {
                self.usage_windows.sections_described_by(|snapshot| {
                    claude_usage_windows_hover_card(snapshot, card_clock)
                })
            }))
            .refreshed_by(ClaudeUsageInteraction::OnBuses(
                ClaudeUsageBusRequest::RefreshUsageWindows,
            ))
    }
}
