use sottomano_desktop_sources::{
    SnapshotSource,
    ai_usage_spend::{
        claude_usage::{ClaudeEnterpriseSpend, ClaudeOrganization, ExtraStatus},
        usage_quota::Quota,
    },
};

use crate::{
    clock::Clock,
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        amount_text::spend_text, localized_quantities::Quantity, ongoing_toast::IndicatorAlerts,
        spend_to_level::level_from_used, value_cells::LabeledValue,
    },
};

pub(crate) const CLAUDE_ENTERPRISE_SPEND_FALLBACK_GLYPH: IconGlyph =
    IconGlyph::Phosphor(Phosphor::Buildings(Weight::Regular));

pub fn claude_enterprise_indicator(
    spend: &ClaudeEnterpriseSpend,
    clock: Clock,
) -> GroupIndicator<'_> {
    let quota_windows = spend.quotas(clock.utc());
    let state = Quota::worst_severity(&quota_windows)
        .unwrap_or_else(|| level_from_used(spend.used_share))
        .into();
    let gauge = match &spend.limit {
        Some(_) => ReadoutGauge::Gauged {
            fill: spend.used_share,
            live: spend.used_share,
        },
        None => ReadoutGauge::Ungauged,
    };
    let panel_visibility = if spend.used.is_positive() || spend.limit.is_some() {
        PanelVisibility::Shown
    } else {
        PanelVisibility::Hidden
    };
    GroupIndicator::glyph_only(
        ClaudeEnterpriseSpend::NAME,
        Phosphor::Buildings(Weight::Regular).into(),
        state,
    )
    .with_readout(IndicatorReadout {
        label: ReadoutLabel::Labeled(format!("{} month", spend_text(&spend.used))),
        gauge,
        badge: ReadoutBadge::Unbadged,
    })
    .with_alerts(IndicatorAlerts {
        quota_windows,
        ..IndicatorAlerts::default()
    })
    .with_panel_visibility(panel_visibility)
    .with_hover_card(Box::new(move |_| claude_enterprise_hover_card(spend)))
}

pub fn claude_enterprise_hover_card(spend: &ClaudeEnterpriseSpend) -> Vec<Section> {
    let (status, reason) = match &spend.status {
        ExtraStatus::Enabled => ("enabled", None),
        ExtraStatus::Disabled { reason } => ("disabled", reason.as_deref()),
    };
    let limit_entries = spend.limit.iter().flat_map(|limit| {
        [
            LabeledValue::new("Monthly Limit", Quantity::spend(*limit)),
            LabeledValue::new(
                "Percent Used",
                Quantity::share(spend.used_share).decimals(2),
            ),
        ]
    });
    vec![Section::new(
        "Usage Credits",
        dictionary_table(
            [LabeledValue::new("Status", status)]
                .into_iter()
                .chain(reason.map(|reason| LabeledValue::new("Reason", reason)))
                .chain([LabeledValue::new(
                    "Used This Month",
                    Quantity::spend(spend.used),
                )])
                .chain(limit_entries),
        ),
    )]
}

pub fn claude_organization_sections(claude_organization: &ClaudeOrganization) -> Vec<Section> {
    vec![Section::new(
        "Subscription",
        dictionary_table([
            LabeledValue::new("Organization", claude_organization.organization.as_str()),
            LabeledValue::new("Seat Tier", claude_organization.seat_tier.as_str()),
            LabeledValue::new(
                "Subscription Status",
                claude_organization.subscription_status.as_str(),
            ),
        ]),
    )]
}
