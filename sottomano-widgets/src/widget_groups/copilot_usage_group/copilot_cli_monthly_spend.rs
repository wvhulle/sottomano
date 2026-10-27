use chrono::{DateTime, Locale, Utc};
use rust_decimal::Decimal;
use sottomano_sources::ai_usage_spend::{
    copilot_usage::CopilotCcusage, spend_money::usd, usage_quota::Quota,
};

use crate::{
    clock::Clock,
    widgets::{
        amount_text::spend_text,
        bar_widget::{BarWidget, Glyph},
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        spend_forecast_basis::MONTH_HEADING_FORMAT,
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
        Phosphor::TerminalWindow(Weight::Regular).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::TerminalWindow(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(if self.period.usage.cost.is_positive() {
            format!("{} cli", spend_text(&self.period.usage.cost))
        } else {
            "cli".to_owned()
        })
    }

    fn state(&self, clock: Clock) -> WidgetState {
        Quota::worst_severity(&self.quotas(clock.utc()))
            .unwrap_or_else(|| level_from_cost(&self.period.usage.cost))
            .into()
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
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
                    Quantity::spend(usd(Decimal::from(self.budget.monthly_limit_usd))),
                )]),
            ),
            Section::dictionary([
                KeyValue::new("Tokens", Quantity::count(usage.tokens)),
                KeyValue::new("Cost", Quantity::spend(usage.cost)),
                KeyValue::new("Models", usage.models.to_string()),
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
