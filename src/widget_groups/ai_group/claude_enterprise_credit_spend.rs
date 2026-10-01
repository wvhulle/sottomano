use uom::si::{f64::Ratio, ratio::percent};

use crate::{
    data::source::ai::claude::{ClaudeEnterpriseSpend, ExtraStatus},
    widget_groups::ai_group::{amount_text::percent_text, spend_to_level::level_from_used},
    widgets::{
        bar_widget::{BarWidget, Glyph, Occasional},
        simple_icons::SimpleIcon,
        tooltip_body::{TooltipBody, row, untitled},
        widget_state::WidgetState,
    },
};

impl BarWidget for ClaudeEnterpriseSpend {
    fn icon(&self) -> Glyph {
        SimpleIcon::Claude.into()
    }

    fn label(&self) -> Option<String> {
        Some(self.spent.short_text())
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
        let rows = [
            Some(row("Plan", self.plan.clone())),
            Some(row("Status", status)),
            Some(row("Spent", self.spent.to_string())),
            self.limit
                .as_ref()
                .map(|limit| row("Limit", limit.to_string())),
            self.limit
                .as_ref()
                .map(|_| row("Used", percent_text(self.used_percent))),
        ]
        .into_iter()
        .flatten()
        .collect();
        TooltipBody::Sections(vec![untitled(rows)])
    }
}

impl Occasional for ClaudeEnterpriseSpend {
    fn present(&self) -> bool {
        self.spent.amount > 0.0 || self.limit.is_some()
    }
}
