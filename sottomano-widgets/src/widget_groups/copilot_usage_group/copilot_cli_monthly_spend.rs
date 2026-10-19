use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sottomano_sources::ai_usage_spend::{
    copilot_usage::CopilotCcusage, spend_money::usd, usage_quota::Quota,
};

use crate::widgets::{
    amount_text::spend_text,
    bar_widget::{BarWidget, Glyph},
    phosphor::{Phosphor, Weight},
    spend_forecast_basis::MONTH_HEADING_FORMAT,
    spend_to_level::level_from_cost,
    tooltip_body::{KeyValue, Section, TooltipBody},
    widget_state::WidgetState,
};

impl BarWidget for CopilotCcusage {
    fn name(&self) -> &'static str {
        "Copilot CLI spend"
    }

    fn icon(&self) -> Glyph {
        Phosphor::TerminalWindow(self.state().weight()).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::TerminalWindow(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        Some(if self.period.usage.cost.is_positive() {
            format!("{} cli", spend_text(&self.period.usage.cost))
        } else {
            "cli".to_owned()
        })
    }

    fn state(&self) -> WidgetState {
        Quota::worst_severity(&self.quotas(Utc::now()))
            .unwrap_or_else(|| level_from_cost(&self.period.usage.cost))
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
                .chain([KeyValue::new(
                    "Monthly Budget",
                    usd(Decimal::from(self.budget.monthly_limit_usd)).to_string(),
                )]),
            ),
            Section::dictionary([
                KeyValue::new("Tokens", usage.tokens.to_string()),
                KeyValue::new("Cost", usage.cost.to_string()),
                KeyValue::new("Models", usage.models.clone()),
            ])
            .titled(
                self.period
                    .span
                    .since
                    .format(MONTH_HEADING_FORMAT)
                    .to_string(),
            ),
        ])
    }

    fn visible_in_bar(&self) -> bool {
        self.period.usage.active()
    }

    fn quota_windows(&self, now: DateTime<Utc>) -> Vec<Quota> {
        self.quotas(now)
    }
}
