use uom::si::{f64::Ratio, ratio::percent};

use crate::{
    data::source::ai::claude::{ClaudeEnterpriseSpend, ExtraStatus},
    widget_groups::ai_group::{amount_text::percent_text, spend_to_level::level_from_used},
    widgets::{
        bar_widget::{BarWidget, Glyph, Occasional},
        simple_icons::SimpleIcon,
        tooltip_body::{TooltipBody, row, section, untitled},
        widget_state::WidgetState,
    },
};

impl BarWidget for ClaudeEnterpriseSpend {
    fn icon(&self) -> Glyph {
        SimpleIcon::Claude.into()
    }

    fn label(&self) -> Option<String> {
        Some(self.used.short_text())
    }

    fn state(&self) -> WidgetState {
        level_from_used(self.used_percent).into()
    }

    fn gauge(&self) -> Option<Ratio> {
        self.limit
            .as_ref()
            .map(|_| Ratio::new::<percent>(self.used_percent))
    }

    fn tooltip(&self) -> TooltipBody {
        let status = match &self.status {
            ExtraStatus::Enabled => "enabled".to_owned(),
            ExtraStatus::Disabled(reason) => reason.clone(),
        };
        let organization = untitled(vec![
            row("Organization", self.organization.clone()),
            row("Seat Tier", self.seat_tier.clone()),
            row("Subscription Status", self.subscription_status.clone()),
        ]);
        let credit_rows = [
            Some(row("Status", status)),
            Some(row("Used", self.used.to_string())),
            self.limit
                .as_ref()
                .map(|limit| row("Limit", limit.to_string())),
            self.limit
                .as_ref()
                .map(|_| row("Percent Used", percent_text(self.used_percent))),
        ]
        .into_iter()
        .flatten()
        .collect();
        TooltipBody::Sections(vec![organization, section("Usage Credits", credit_rows)])
    }
}

impl Occasional for ClaudeEnterpriseSpend {
    fn present(&self) -> bool {
        self.used.amount > 0.0 || self.limit.is_some()
    }
}
