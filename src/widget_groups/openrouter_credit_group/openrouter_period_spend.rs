use chrono::{DateTime, Local, Utc};

use crate::{
    data::{
        severity::Severity,
        source::ai::{
            openrouter::{OpenRouterSpend, SpanSpend},
            quota::QuotaLimited,
        },
    },
    widgets::{
        amount_text::{dollars, to_cents},
        bar_widget::{BarWidget, Glyph, Occasional},
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

const LISTED_MODELS: usize = 6;

impl BarWidget for OpenRouterSpend {
    fn name(&self) -> &'static str {
        "OpenRouter spend"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Coins(self.state().weight()).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::Coins(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        Some(format!("{} today", dollars(self.today.cost())))
    }

    fn state(&self) -> WidgetState {
        let daily = if self.today.cost() >= f64::from(self.daily_warning_usd) {
            Severity::Warning
        } else {
            Severity::Normal
        };
        self.quota_severity(Utc::now())
            .map_or(daily, |monthly| monthly.max(daily))
            .into()
    }

    fn tooltip(&self) -> TooltipBody {
        let now = Local::now();
        TooltipBody::sections([
            Section::dictionary(
                [KeyValue::new("Source", "OpenRouter analytics")]
                    .into_iter()
                    .chain(self.budget.forecast_basis())
                    .chain([KeyValue::new(
                        "Daily Warning",
                        dollars(f64::from(self.daily_warning_usd)),
                    )]),
            ),
            self.span_section("Today".to_owned(), &self.today, now),
            self.span_section(self.week.span.week_heading(), &self.week, now),
            self.span_section(self.month.span.month_heading(), &self.month, now),
        ])
    }
}

impl Occasional for OpenRouterSpend {
    fn present(&self) -> bool {
        self.month.cost() > 0.0
    }
}

impl OpenRouterSpend {
    fn span_section(&self, title: String, spent: &SpanSpend, now: DateTime<Local>) -> Section {
        let projected = spent
            .projected_cost(now, &self.budget)
            .map(|cost| KeyValue::new("Projected Cost", dollars(cost)));
        let rest: f64 = spent
            .models
            .iter()
            .skip(LISTED_MODELS)
            .map(|model| model.cost)
            .sum();
        let other = (rest > 0.0).then(|| KeyValue::new("other", format!("${}", to_cents(rest))));
        Section::dictionary(
            [
                Some(KeyValue::new("Cost", dollars(spent.cost()))),
                projected,
            ]
            .into_iter()
            .flatten()
            .chain(spent.models.iter().take(LISTED_MODELS).map(|model| {
                KeyValue::new(model.model.as_str(), format!("${}", to_cents(model.cost)))
            }))
            .chain(other),
        )
        .titled(title)
    }
}
