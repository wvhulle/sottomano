use chrono::{DateTime, Local, Utc};
use rust_decimal::Decimal;

use crate::{
    data::{
        money::{Money, usd},
        severity::Severity,
        source::ai::{
            openrouter::{OpenRouterSpend, SpanSpend},
            quota::QuotaLimited,
        },
    },
    widgets::{
        amount_text::spend_text,
        bar_widget::{BarWidget, Glyph, Occasional},
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for OpenRouterSpend {
    fn name(&self) -> &'static str {
        "OpenRouter soft budget"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Coins(self.state().weight()).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::Coins(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        let today = self.today.cost();
        today
            .is_positive()
            .then(|| format!("{} today", spend_text(&today)))
    }

    fn state(&self) -> WidgetState {
        self.quota_severity(Utc::now())
            .map_or(self.daily_severity(), |monthly| {
                monthly.max(self.daily_severity())
            })
            .into()
    }

    fn tooltip(&self) -> TooltipBody {
        let now = Local::now();
        let daily_warning = KeyValue::new("Daily Warning", self.daily_warning().to_string())
            .level(self.daily_severity());
        let monthly_budget = self.budget.monthly_budget_entry(
            &self.month.cost(),
            self.month.projected_cost(now, &self.budget),
        );
        TooltipBody::sections([
            Section::dictionary(
                [KeyValue::new("Source", "OpenRouter analytics")]
                    .into_iter()
                    .chain(self.budget.forecast_basis()),
            ),
            self.span_section("Today".to_owned(), &self.today, Some(daily_warning), now),
            self.span_section(self.week.span.week_heading(), &self.week, None, now),
            self.span_section(
                self.month.span.month_heading(),
                &self.month,
                Some(monthly_budget),
                now,
            ),
        ])
    }
}

impl Occasional for OpenRouterSpend {
    fn present(&self) -> bool {
        self.month.cost().is_positive()
    }
}

impl OpenRouterSpend {
    fn daily_warning(&self) -> Money {
        usd(Decimal::from(self.daily_warning_usd))
    }

    fn daily_severity(&self) -> Severity {
        if self.today.cost().amount() >= self.daily_warning().amount() {
            Severity::Warning
        } else {
            Severity::Normal
        }
    }

    fn span_section(
        &self,
        title: String,
        spent: &SpanSpend,
        threshold: Option<KeyValue>,
        now: DateTime<Local>,
    ) -> Section {
        let projected = spent
            .projected_cost(now, &self.budget)
            .map(|cost| KeyValue::new("Projected Cost", cost.to_string()));
        Section::dictionary(
            [
                Some(KeyValue::new("Cost", spent.cost().to_string())),
                projected,
                threshold,
            ]
            .into_iter()
            .flatten(),
        )
        .titled(title)
    }
}
