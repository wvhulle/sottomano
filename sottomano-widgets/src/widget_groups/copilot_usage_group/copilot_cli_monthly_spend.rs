use chrono::{DateTime, Locale, Utc};
use rust_decimal::Decimal;
use sottomano_desktop_sources::ai_usage_spend::{
    copilot_usage::CopilotCcusage, spend_money::usd, usage_quota::Quota,
};

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading, reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widgets::{
        amount_text::spend_text,
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        spend_forecast_basis::MONTH_HEADING_FORMAT,
        spend_to_level::level_from_cost,
        value_cells::LabeledValue,
    },
};

impl GroupItemReading for CopilotCcusage {
    fn name(&self) -> &'static str {
        "Copilot CLI spend"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::TerminalWindow(Weight::Regular).into()
    }

    fn fallback_icon() -> IconGlyph {
        Phosphor::TerminalWindow(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(if self.period.usage.cost.is_positive() {
            format!("{} cli", spend_text(&self.period.usage.cost))
        } else {
            "cli".to_owned()
        })
    }

    fn state(&self, clock: Clock) -> ReadingDisplayState {
        Quota::worst_severity(&self.quotas(clock.utc()))
            .unwrap_or_else(|| level_from_cost(&self.period.usage.cost))
            .into()
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let usage = &self.period.usage;
        HoverCardBody::sections([
            Section::dictionary(
                [
                    LabeledValue::new("Plan", "copilot-cli"),
                    LabeledValue::new("Source", "ccusage, local CLI session data"),
                ]
                .into_iter()
                .chain([LabeledValue::new(
                    "Monthly Budget",
                    Quantity::spend(usd(Decimal::from(self.budget.monthly_limit_usd))),
                )]),
            ),
            Section::dictionary([
                LabeledValue::new("Tokens", Quantity::count(usage.tokens)),
                LabeledValue::new("Cost", Quantity::spend(usage.cost)),
                LabeledValue::new("Models", usage.models.to_string()),
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

    fn visible_on_panel(&self) -> bool {
        self.period.usage.active()
    }

    fn quota_windows(&self, now: DateTime<Utc>) -> Vec<Quota> {
        self.quotas(now)
    }
}
