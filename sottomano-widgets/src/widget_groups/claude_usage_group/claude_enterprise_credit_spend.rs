use sottomano_desktop_sources::{
    SnapshotSource,
    ai_usage_spend::{
        claude_usage::{ClaudeEnterpriseSpend, ClaudeOrganization, ExtraStatus},
        usage_quota::Quota,
    },
};

use crate::{
    clock::Clock,
    group_items::{
        reading_row::{PanelVisibility, ReadingRow},
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{ReadingSections, Section, dictionary_table},
    widgets::{
        amount_text::spend_text,
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        ongoing_toast::ReadingAlerts,
        phosphor::{Phosphor, Weight},
        spend_to_level::level_from_used,
        value_cells::LabeledValue,
    },
};

pub(crate) const CLAUDE_ENTERPRISE_SPEND_FALLBACK_GLYPH: IconGlyph =
    IconGlyph::Phosphor(Phosphor::Buildings(Weight::Regular));

pub fn claude_enterprise_reading_row(
    spend: &ClaudeEnterpriseSpend,
    clock: Clock,
) -> ReadingRow<'_> {
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
    ReadingRow {
        readout: RowReadout {
            label: ReadoutLabel::Labeled(format!("{} month", spend_text(&spend.used))),
            gauge,
            badge: ReadoutBadge::Unbadged,
        },
        alerts: ReadingAlerts {
            quota_windows,
            ..ReadingAlerts::default()
        },
        panel_visibility,
        ..ReadingRow::glyph_only(
            ClaudeEnterpriseSpend::NAME,
            Phosphor::Buildings(Weight::Regular).into(),
            state,
            Box::new(move |card_clock| claude_enterprise_hover_card(spend, card_clock)),
        )
    }
}

pub fn claude_enterprise_hover_card(spend: &ClaudeEnterpriseSpend, _clock: Clock) -> Vec<Section> {
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

impl ReadingSections for ClaudeOrganization {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::new(
            "Subscription",
            dictionary_table([
                LabeledValue::new("Organization", self.organization.as_str()),
                LabeledValue::new("Seat Tier", self.seat_tier.as_str()),
                LabeledValue::new("Subscription Status", self.subscription_status.as_str()),
            ]),
        )]
    }
}
