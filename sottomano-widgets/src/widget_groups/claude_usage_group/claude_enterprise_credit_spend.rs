use chrono::{DateTime, Locale, Utc};
use sottomano_sources::ai_usage_spend::{
    claude_usage::{ClaudeEnterpriseSpend, ClaudeOrganization, ExtraStatus},
    usage_quota::Quota,
};
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    widgets::{
        amount_text::spend_text,
        bar_widget::{BarWidget, Glyph},
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        spend_to_level::level_from_used,
        tooltip_body::{ReadingSections, Section, TooltipBody},
        value_cells::LabeledValue,
        widget_state::WidgetState,
    },
};

impl BarWidget for ClaudeEnterpriseSpend {
    fn name(&self) -> &'static str {
        "Claude Enterprise spend"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Buildings(Weight::Regular).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::Buildings(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(format!("{} month", spend_text(&self.used)))
    }

    fn state(&self, clock: Clock) -> WidgetState {
        Quota::worst_severity(&self.quotas(clock.utc()))
            .unwrap_or_else(|| level_from_used(self.used_share))
            .into()
    }

    fn gauge(&self) -> Option<Ratio> {
        self.limit.as_ref().map(|_| self.used_share)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
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
        TooltipBody::sections([Section::dictionary(
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

    fn visible_in_bar(&self) -> bool {
        self.used.is_positive() || self.limit.is_some()
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
