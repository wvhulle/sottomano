use chrono::{DateTime, Locale, Utc};
use sottomano_desktop_sources::ai_usage_spend::{
    openrouter_credits::{OpenRouterSpend, SpanSpend},
    usage_quota::Quota,
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
        spend_forecast_basis::{MONTH_HEADING_FORMAT, WEEK_HEADING_FORMAT},
        value_cells::LabeledValue,
    },
};

impl GroupItemReading for OpenRouterSpend {
    fn name(&self) -> &'static str {
        "OpenRouter soft budget"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Coins(Weight::Regular).into()
    }

    fn fallback_icon() -> IconGlyph {
        Phosphor::Coins(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        let today = self.today.cost();
        today
            .is_positive()
            .then(|| format!("{} today", spend_text(&today)))
    }

    fn state(&self, clock: Clock) -> ReadingDisplayState {
        Quota::worst_severity(&self.quotas(clock.utc()))
            .map_or(self.daily_severity(), |monthly| {
                monthly.max(self.daily_severity())
            })
            .into()
    }

    fn hover_card(&self, clock: Clock) -> HoverCardBody {
        let now = clock.local();
        let daily_warning =
            LabeledValue::new("Daily Warning", Quantity::spend(self.daily_warning()))
                .level(self.daily_severity());
        let monthly_budget = LabeledValue::monthly_budget(
            &self.budget,
            &self.month.cost(),
            self.month.projected_cost(now, &self.budget),
        );
        let span_section = |title: String, spent: &SpanSpend, threshold: Option<LabeledValue>| {
            let projected = spent
                .projected_cost(now, &self.budget)
                .map(|cost| LabeledValue::new("Projected Cost", Quantity::spend(cost)));
            Section::dictionary(
                [
                    Some(LabeledValue::new("Cost", Quantity::spend(spent.cost()))),
                    projected,
                    threshold,
                ]
                .into_iter()
                .flatten(),
            )
            .titled(title)
        };
        HoverCardBody::sections([
            Section::spend_forecast_basis(
                [LabeledValue::new("Source", "OpenRouter analytics")],
                &self.budget,
            ),
            span_section("Today".to_owned(), &self.today, Some(daily_warning)),
            span_section(
                self.week.span.since.format(WEEK_HEADING_FORMAT).to_string(),
                &self.week,
                None,
            ),
            span_section(
                self.month
                    .span
                    .since
                    .format(MONTH_HEADING_FORMAT)
                    .to_string(),
                &self.month,
                Some(monthly_budget),
            ),
        ])
    }

    fn visible_on_panel(&self) -> bool {
        self.month.cost().is_positive()
    }

    fn quota_windows(&self, now: DateTime<Utc>) -> Vec<Quota> {
        self.quotas(now)
    }
}
