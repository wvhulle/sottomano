use chrono::Utc;
use uom::si::f64::Ratio;

use crate::{
    data::source::ai::{
        claude::{ClaudeEnterpriseSpend, ClaudeOrganization, ExtraStatus},
        quota::QuotaLimited,
    },
    widget_groups::ai_group::{amount_text::percent_text, spend_to_level::level_from_used},
    widgets::{
        bar_widget::{BarWidget, Glyph, Occasional},
        simple_icons::SimpleIcon,
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for ClaudeEnterpriseSpend {
    fn name(&self) -> &'static str {
        "Claude Enterprise spend"
    }

    fn icon(&self) -> Glyph {
        SimpleIcon::Claude.into()
    }

    fn label(&self) -> Option<String> {
        Some(self.used.short_text())
    }

    fn state(&self) -> WidgetState {
        self.quota_severity(Utc::now())
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
        TooltipBody::sections(&[Section::dictionary(
            "Usage Credits",
            [KeyValue::new("Status", status)]
                .into_iter()
                .chain(reason.map(|reason| KeyValue::new("Reason", reason)))
                .chain([KeyValue::new("Used This Month", self.used.to_string())])
                .chain(limit_entries),
        )])
    }
}

impl ClaudeOrganization {
    pub fn sections(&self) -> Vec<Section> {
        vec![Section::untitled_dictionary([
            KeyValue::new("Organization", self.organization.as_str()),
            KeyValue::new("Seat Tier", self.seat_tier.as_str()),
            KeyValue::new("Subscription Status", self.subscription_status.as_str()),
        ])]
    }
}

impl Occasional for ClaudeEnterpriseSpend {
    fn present(&self) -> bool {
        self.used.amount > 0.0 || self.limit.is_some()
    }
}
