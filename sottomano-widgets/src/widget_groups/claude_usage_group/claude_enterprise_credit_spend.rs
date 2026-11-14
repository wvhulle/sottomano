use std::borrow::Cow;

use chrono::{DateTime, Locale, Utc};
use sottomano_desktop_sources::{
    SnapshotSource,
    ai_usage_spend::{
        claude_usage::{ClaudeEnterpriseSpend, ClaudeOrganization, ExtraStatus},
        usage_quota::Quota,
    },
};
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::{GroupItemReading, PanelVisibility},
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, ReadingSections, Section},
    widgets::{
        amount_text::spend_text,
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        spend_to_level::level_from_used,
        value_cells::LabeledValue,
    },
};

impl GroupItemReading for ClaudeEnterpriseSpend {
    fn name(&self) -> Cow<'static, str> {
        Self::NAME.into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Buildings(Weight::Regular).into()
    }

    fn fallback_icon() -> IconGlyph {
        Phosphor::Buildings(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(format!("{} month", spend_text(&self.used)))
    }

    fn state(&self, clock: Clock) -> ReadingDisplayState {
        Quota::worst_severity(&self.quotas(clock.utc()))
            .unwrap_or_else(|| level_from_used(self.used_share))
            .into()
    }

    fn gauge(&self) -> Option<Ratio> {
        self.limit.as_ref().map(|_| self.used_share)
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let (status, reason) = match &self.status {
            ExtraStatus::Enabled => ("enabled", None),
            ExtraStatus::Disabled { reason } => ("disabled", reason.as_deref()),
        };
        let limit_entries = self.limit.iter().flat_map(|limit| {
            [
                LabeledValue::new("Monthly Limit", Quantity::spend(*limit)),
                LabeledValue::new("Percent Used", Quantity::share(self.used_share).decimals(2)),
            ]
        });
        HoverCardBody::sections([Section::dictionary(
            [LabeledValue::new("Status", status)]
                .into_iter()
                .chain(reason.map(|reason| LabeledValue::new("Reason", reason)))
                .chain([LabeledValue::new(
                    "Used This Month",
                    Quantity::spend(self.used),
                )])
                .chain(limit_entries),
        )
        .titled("Usage Credits")])
    }

    fn panel_visibility(&self) -> PanelVisibility {
        if self.used.is_positive() || self.limit.is_some() {
            PanelVisibility::Shown
        } else {
            PanelVisibility::Hidden
        }
    }

    fn quota_windows(&self, now: DateTime<Utc>) -> Vec<Quota> {
        self.quotas(now)
    }
}

impl ReadingSections for ClaudeOrganization {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::dictionary([
            LabeledValue::new("Organization", self.organization.as_str()),
            LabeledValue::new("Seat Tier", self.seat_tier.as_str()),
            LabeledValue::new("Subscription Status", self.subscription_status.as_str()),
        ])]
    }
}
