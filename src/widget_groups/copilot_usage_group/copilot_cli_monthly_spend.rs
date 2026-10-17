use chrono::Utc;
use rust_decimal::Decimal;

use crate::{
    data::{
        money::usd,
        source::ai::{copilot::CopilotCcusage, quota::QuotaLimited},
    },
    widgets::{
        amount_text::spend_text,
        bar_widget::{BarWidget, Glyph, Occasional},
        phosphor::{Phosphor, Weight},
        spend_to_level::level_from_cost,
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
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
        self.quota_severity(Utc::now())
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
            .titled(self.period.span.month_heading()),
        ])
    }
}

impl Occasional for CopilotCcusage {
    fn present(&self) -> bool {
        self.period.usage.active()
    }
}
