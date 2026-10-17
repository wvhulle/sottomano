use chrono::{DateTime, Local, Utc};

use crate::{
    data::source::ai::{ccusage::Period, claude::ClaudeCcusagePeriods, quota::QuotaLimited},
    widgets::{
        amount_text::{spend_text, token_count},
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
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
        Phosphor::CalendarDots(self.state().weight()).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::CalendarDots(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        Some(format!(
            "{} today",
            spend_text(&self.recent.today.usage.cost)
        ))
    }

    fn state(&self) -> WidgetState {
        self.quota_severity(Utc::now())
            .unwrap_or_else(|| level_from_cost(&self.recent.today.usage.cost))
            .into()
    }

    fn tooltip(&self) -> TooltipBody {
        let recent = &self.recent;
        let now = Local::now();
        TooltipBody::sections([
            Section::dictionary(
                [
                    KeyValue::new("Source", "ccusage, local Claude Code logs"),
                    KeyValue::new("Cost Basis", "API list prices"),
                ]
                .into_iter()
                .chain(self.budget.forecast_basis()),
            ),
            self.period_section("Today".to_owned(), &recent.today, None, now),
            self.period_section(recent.week.span.week_heading(), &recent.week, None, now),
            self.period_section(
                recent.month.span.month_heading(),
                &recent.month,
                Some(self.budget.monthly_budget_entry(
                    &recent.month.usage.cost,
                    recent.month.projected_cost(now, &self.budget),
                )),
                now,
            ),
        ])
    }
}

impl ClaudeCcusagePeriods {
    fn period_section(
        &self,
        title: String,
        period: &Period,
        threshold: Option<KeyValue>,
        now: DateTime<Local>,
    ) -> Section {
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
    }
}
