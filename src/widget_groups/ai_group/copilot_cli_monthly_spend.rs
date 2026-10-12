use chrono::Utc;

use crate::{
    data::source::ai::{copilot::CopilotCcusage, quota::QuotaLimited},
    widget_groups::ai_group::{amount_text::dollars, spend_to_level::level_from_cost},
    widgets::{
        bar_widget::{BarWidget, Glyph, Occasional},
        simple_icons::SimpleIcon,
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for CopilotCcusage {
    fn name(&self) -> &'static str {
        "Copilot CLI spend"
    }

    fn icon(&self) -> Glyph {
        SimpleIcon::GithubCopilot.into()
    }

    fn label(&self) -> Option<String> {
        Some(if self.period.usage.cost > 0.0 {
            dollars(self.period.usage.cost)
        } else {
            "cli".to_owned()
        })
    }

    fn state(&self) -> WidgetState {
        self.quota_severity(Utc::now())
            .unwrap_or_else(|| level_from_cost(self.period.usage.cost))
            .into()
    }

    fn tooltip(&self) -> TooltipBody {
        let usage = &self.period.usage;
        TooltipBody::sections([
            Section::dictionary(
                [
                    KeyValue::new("Plan", "copilot-cli"),
                    KeyValue::new("Source", "ccusage, local CLI session data"),
                ]
                .into_iter()
                .chain(
                    self.budget
                        .monthly_limit_usd
                        .map(|limit| KeyValue::new("Monthly Budget", dollars(f64::from(limit)))),
                ),
            ),
            Section::dictionary([
                KeyValue::new("Tokens", usage.tokens.to_string()),
                KeyValue::new("Cost", dollars(usage.cost)),
                KeyValue::new("Models", usage.models.clone()),
            ])
            .titled(self.month.clone()),
        ])
    }
}

impl Occasional for CopilotCcusage {
    fn present(&self) -> bool {
        self.period.usage.active()
    }
}
