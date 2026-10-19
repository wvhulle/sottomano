use chrono::{DateTime, Locale, Utc};
use sottomano_sources::ai_usage_spend::{
    ccusage_cli_reports::Period, claude_usage::ClaudeCcusagePeriods, usage_quota::Quota,
};

use crate::{
    clock::Clock,
    widgets::{
        amount_text::{spend_text, token_count},
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        spend_forecast_basis::{MONTH_HEADING_FORMAT, WEEK_HEADING_FORMAT},
        spend_to_level::level_from_cost,
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for ClaudeCcusagePeriods {
    fn name(&self) -> &'static str {
        "Claude Code spend"
    }

    fn icon(&self) -> Glyph {
        Phosphor::CalendarDots(Weight::Regular).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::CalendarDots(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(format!(
            "{} today",
            spend_text(&self.recent.today.usage.cost)
        ))
    }

    fn state(&self, clock: Clock) -> WidgetState {
        Quota::worst_severity(&self.quotas(clock.utc()))
            .unwrap_or_else(|| level_from_cost(&self.recent.today.usage.cost))
            .into()
    }

    fn tooltip(&self, clock: Clock) -> TooltipBody {
        let recent = &self.recent;
        let now = clock.local();
        let period_section = |title: String, period: &Period, threshold: Option<KeyValue>| {
            let projected = period
                .projected_cost(now, &self.budget)
                .map(|cost| KeyValue::new("Projected Cost", cost.to_string()));
            Section::dictionary(
                [
                    Some(KeyValue::new("Cost", period.usage.cost.to_string())),
                    projected,
                    threshold,
                    Some(KeyValue::new("Tokens", token_count(period.usage.tokens))),
                    Some(KeyValue::new("Models", period.usage.models.clone())),
                ]
                .into_iter()
                .flatten(),
            )
            .titled(title)
        };
        TooltipBody::sections([
            Section::spend_forecast_basis(
                [
                    KeyValue::new("Source", "ccusage, local Claude Code logs"),
                    KeyValue::new("Cost Basis", "API list prices"),
                ],
                &self.budget,
            ),
            period_section("Today".to_owned(), &recent.today, None),
            period_section(
                recent
                    .week
                    .span
                    .since
                    .format(WEEK_HEADING_FORMAT)
                    .to_string(),
                &recent.week,
                None,
            ),
            period_section(
                recent
                    .month
                    .span
                    .since
                    .format(MONTH_HEADING_FORMAT)
                    .to_string(),
                &recent.month,
                Some(KeyValue::monthly_budget(
                    &self.budget,
                    &recent.month.usage.cost,
                    recent.month.projected_cost(now, &self.budget),
                )),
            ),
        ])
    }

    fn quota_windows(&self, now: DateTime<Utc>) -> Vec<Quota> {
        self.quotas(now)
    }
}
