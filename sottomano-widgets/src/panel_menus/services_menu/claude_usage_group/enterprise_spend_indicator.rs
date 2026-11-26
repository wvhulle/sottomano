use sottomano_desktop_sources::{
    SnapshotSource,
    ai_usage_spend::{
        claude_usage::{ClaudeEnterpriseSpend, ClaudeOrganization, ExtraStatus, SpendLimit},
        usage_quota::Quota,
    },
};

use super::{ClaudeUsageBusRequest, ClaudeUsageGroup, ClaudeUsageInteraction};
use crate::{
    clock::Clock,
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_group_content::IndicatorListing,
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
    let gauge = match spend.limit {
        SpendLimit::Capped(_) => ReadoutGauge::Gauged {
            fill: spend.used_share,
            live: spend.used_share,
        },
        SpendLimit::Uncapped => ReadoutGauge::Ungauged,
    };
    let panel_visibility = match spend.limit {
        SpendLimit::Capped(_) => PanelVisibility::Shown,
        SpendLimit::Uncapped if spend.used.is_positive() => PanelVisibility::Shown,
        SpendLimit::Uncapped => PanelVisibility::Hidden,
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
    let mut credit_rows = extra_status_rows(&spend.status);
    credit_rows.push(LabeledValue::new(
        "Used This Month",
        Quantity::spend(spend.used),
    ));
    if let SpendLimit::Capped(limit) = spend.limit {
        credit_rows.push(LabeledValue::new("Monthly Limit", Quantity::spend(limit)));
        credit_rows.push(LabeledValue::new(
            "Percent Used",
            Quantity::share(spend.used_share).decimals(2),
        ));
    }
    vec![Section::new("Usage Credits", dictionary_table(credit_rows))]
}

pub(crate) fn extra_status_rows(status: &ExtraStatus) -> Vec<LabeledValue> {
    match status {
        ExtraStatus::Enabled => vec![LabeledValue::new("Status", "enabled")],
        ExtraStatus::Disabled => vec![LabeledValue::new("Status", "disabled")],
        ExtraStatus::DisabledBecause(reason) => vec![
            LabeledValue::new("Status", "disabled"),
            LabeledValue::new("Reason", reason.as_str()),
        ],
    }
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

impl ClaudeUsageGroup {
    pub(super) fn enterprise_spend_listing(&self, clock: Clock) -> IndicatorListing<'_> {
        self.enterprise_spend
            .indicator_listing(
                |held_snapshot| claude_enterprise_indicator(held_snapshot, clock).when_shown(),
                CLAUDE_ENTERPRISE_SPEND_FALLBACK_GLYPH,
            )
            .map_indicators(|row| self.enterprise_spend_indicator(row))
    }

    fn enterprise_spend_indicator<'view>(
        &'view self,
        spend_row: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        spend_row
            .with_hover_card(Box::new(move |_| {
                let mut sections = self
                    .enterprise_spend
                    .sections_described_by(claude_enterprise_hover_card);
                sections.extend(
                    self.organization
                        .sections_described_by(claude_organization_sections),
                );
                sections
            }))
            .fetching_on_hover(
                ClaudeUsageInteraction::OnBuses(ClaudeUsageBusRequest::StartOrganizationFetch),
                ClaudeUsageInteraction::StopHoverFetches,
            )
            .refreshed_by(ClaudeUsageInteraction::OnBuses(
                ClaudeUsageBusRequest::RefreshEnterpriseSpend,
            ))
    }
}
