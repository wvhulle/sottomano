use chrono::{DateTime, Local, Utc};

use crate::{
    data::source::ai::{ccusage::Period, claude::ClaudeCcusagePeriods, quota::QuotaLimited},
    widget_groups::ai_group::{
        amount_text::{dollars, token_count},
        spend_to_level::level_from_cost,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

const WEEK_START_FORMAT: &str = "%a %-d %b";
const MONTH_FORMAT: &str = "%B";
const CLOCK_FORMAT: &str = "%H:%M";

impl BarWidget for ClaudeCcusagePeriods {
    fn name(&self) -> &'static str {
        "Claude Code spend"
    }

    fn icon(&self) -> Glyph {
        Phosphor::TerminalWindow(self.state().weight()).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::TerminalWindow(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        Some(dollars(self.recent.today.usage.cost))
    }

    fn state(&self) -> WidgetState {
        self.quota_severity(Utc::now())
            .unwrap_or_else(|| level_from_cost(self.recent.today.usage.cost))
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
                    KeyValue::new("Working Days", self.working_days_text()),
                    KeyValue::new("Working Hours", self.working_hours_text()),
                    KeyValue::new("Projection", "cost per working hour elapsed so far"),
                ]
                .into_iter()
                .chain(
                    self.budget
                        .monthly_limit_usd
                        .map(|limit| KeyValue::new("Monthly Budget", dollars(f64::from(limit)))),
                ),
            ),
            self.period_section("Today".to_owned(), &recent.today, now),
            self.period_section(
                format!("Week Since {}", recent.week.since.format(WEEK_START_FORMAT)),
                &recent.week,
                now,
            ),
            self.period_section(
                recent.month.since.format(MONTH_FORMAT).to_string(),
                &recent.month,
                now,
            ),
        ])
    }
}

impl ClaudeCcusagePeriods {
    fn working_days_text(&self) -> String {
        self.budget
            .working_days
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn working_hours_text(&self) -> String {
        let hours = &self.budget.working_hours;
        format!(
            "{}–{}",
            hours.start.format(CLOCK_FORMAT),
            hours.end.format(CLOCK_FORMAT)
        )
    }

    fn period_section(&self, title: String, period: &Period, now: DateTime<Local>) -> Section {
        let projected = period
            .projected_cost(now, &self.budget)
            .map(|cost| KeyValue::new("Projected Cost", dollars(cost)));
        Section::dictionary(
            [
                Some(KeyValue::new("Cost", dollars(period.usage.cost))),
                projected,
                Some(KeyValue::new("Tokens", token_count(period.usage.tokens))),
                Some(KeyValue::new("Models", period.usage.models.clone())),
            ]
            .into_iter()
            .flatten(),
        )
        .titled(title)
    }
}
