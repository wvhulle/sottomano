use chrono::{DateTime, Utc};
use sottomano_sources::ai_usage_spend::{
    claude_usage::{ClaudeEnterpriseSpend, ClaudeOrganization, ExtraStatus},
    usage_quota::Quota,
};
use uom::si::f64::Ratio;

use crate::widgets::{
    amount_text::{percent_text, spend_text},
    bar_widget::{BarWidget, Glyph},
    phosphor::{Phosphor, Weight},
    spend_to_level::level_from_used,
    tooltip_body::{KeyValue, ReadingSections, Section, TooltipBody},
    widget_state::WidgetState,
};

impl BarWidget for ClaudeEnterpriseSpend {
    fn name(&self) -> &'static str {
        "Claude Enterprise spend"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Buildings(self.state().weight()).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::Buildings(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        Some(format!("{} month", spend_text(&self.used)))
    }

    fn state(&self) -> WidgetState {
        Quota::worst_severity(&self.quotas(Utc::now()))
            .unwrap_or_else(|| level_from_used(self.used_share))
            .into()
    }

    fn gauge(&self) -> Option<Ratio> {
        self.limit.as_ref().map(|_| self.used_share)
    }

    fn tooltip(&self) -> TooltipBody {
        let (status, reason) = match &self.status {
            ExtraStatus::Enabled => ("enabled", None),
            ExtraStatus::Disabled { reason } => ("disabled", reason.as_deref()),
        };
        let limit_entries = self.limit.iter().flat_map(|limit| {
            [
                KeyValue::new("Monthly Limit", limit.to_string()),
                KeyValue::new("Percent Used", percent_text(self.used_share)),
            ]
        });
        TooltipBody::sections([Section::dictionary(
            [KeyValue::new("Status", status)]
                .into_iter()
                .chain(reason.map(|reason| KeyValue::new("Reason", reason)))
                .chain([KeyValue::new("Used This Month", self.used.to_string())])
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
    fn sections(&self) -> Vec<Section> {
        vec![Section::dictionary([
            KeyValue::new("Organization", self.organization.as_str()),
            KeyValue::new("Seat Tier", self.seat_tier.as_str()),
            KeyValue::new("Subscription Status", self.subscription_status.as_str()),
        ])]
    }
}
