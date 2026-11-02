use chrono::{DateTime, Locale, Utc};
use sottomano_sources::ai_usage_spend::{
    ccusage_cli_reports::Period, claude_usage::ClaudeCcusagePeriods, usage_quota::Quota,
};

use crate::{
    clock::Clock,
    widgets::{
        amount_text::spend_text,
        group_item_reading::GroupItemReading,
        hover_card_body::{HoverCardBody, Section},
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        reading_display_state::ReadingDisplayState,
        spend_forecast_basis::{MONTH_HEADING_FORMAT, WEEK_HEADING_FORMAT},
        spend_to_level::level_from_cost,
        value_cells::LabeledValue,
    },
};

impl GroupItemReading for ClaudeCcusagePeriods {
    fn name(&self) -> &'static str {
        "Claude Code spend"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::CalendarDots(Weight::Regular).into()
    }

    fn fallback_icon() -> IconGlyph {
        Phosphor::CalendarDots(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(format!(
            "{} today",
            spend_text(&self.recent.today.usage.cost)
        ))
    }

    fn state(&self, clock: Clock) -> ReadingDisplayState {
        Quota::worst_severity(&self.quotas(clock.utc()))
            .unwrap_or_else(|| level_from_cost(&self.recent.today.usage.cost))
            .into()
    }

    fn hover_card(&self, clock: Clock) -> HoverCardBody {
        let recent = &self.recent;
        let now = clock.local();
        let period_section = |title: String, period: &Period, threshold: Option<LabeledValue>| {
            let projected = period
                .projected_cost(now, &self.budget)
                .map(|cost| LabeledValue::new("Projected Cost", Quantity::spend(cost)));
            Section::dictionary(
                [
                    Some(LabeledValue::new(
                        "Cost",
                        Quantity::spend(period.usage.cost),
                    )),
                    projected,
                    threshold,
                    Some(LabeledValue::new(
                        "Tokens",
                        Quantity::compact_count(period.usage.tokens),
                    )),
                    Some(LabeledValue::new("Models", period.usage.models.to_string())),
                ]
                .into_iter()
                .flatten(),
            )
            .titled(title)
        };
        HoverCardBody::sections([
            Section::spend_forecast_basis(
                [
                    LabeledValue::new("Source", "ccusage, local Claude Code logs"),
                    LabeledValue::new("Cost Basis", "API list prices"),
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
                Some(LabeledValue::monthly_budget(
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
