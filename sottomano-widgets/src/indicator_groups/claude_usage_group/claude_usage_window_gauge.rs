use std::iter::once;

use chrono::{DateTime, Utc};
use sottomano_desktop_sources::{
    SnapshotSource,
    ai_usage_spend::{
        claude_usage::{ClaudeUsageWindows, ExtraStatus, ExtraUsage, WindowUsage},
        usage_quota::{Pace, Quota},
    },
};
use uom::si::ratio::percent;

use crate::{
    clock::Clock,
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::Quantity,
        ongoing_toast::IndicatorAlerts,
        phosphor::{Phosphor, Weight},
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
    let sonnet = windows.sonnet.as_ref().map(|sonnet| {
        window_section(
            "Weekly Sonnet",
            sonnet,
            windows.sonnet_quota(now).as_ref(),
            now,
        )
    });
    [
        Section::new(
            "Subscription",
            dictionary_table(
                windows
                    .plan
                    .as_deref()
                    .map(|plan| LabeledValue::new("Plan", plan)),
            ),
        ),
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
    ]
    .into_iter()
    .chain(sonnet)
    .chain([extra_section(windows.extra.as_ref())])
    .collect::<Vec<_>>()
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
    Section::new("Extra Usage", dictionary_table(entries))
}
