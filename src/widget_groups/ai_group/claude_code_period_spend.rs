use chrono::NaiveDate;

use crate::{
    data::source::ai::{ccusage::Period, claude::ClaudeCcusagePeriods},
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

impl BarWidget for ClaudeCcusagePeriods {
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
        level_from_cost(self.recent.today.usage.cost).into()
    }

    fn tooltip(&self) -> TooltipBody {
        let recent = &self.recent;
        let today = recent.today.since;
        TooltipBody::sections(&[
            Section::untitled_dictionary([
                KeyValue::new("Source", "ccusage, local Claude Code logs"),
                KeyValue::new("Cost Basis", "API list prices"),
                KeyValue::new("Working Days", self.working_days_text()),
                KeyValue::new("Projection", "cost per working day so far, today included"),
            ]),
            Section::dictionary(
                "Today",
                [
                    KeyValue::new("Cost", dollars(recent.today.usage.cost)),
                    KeyValue::new("Tokens", token_count(recent.today.usage.tokens)),
                    KeyValue::new("Models", recent.today.usage.models.clone()),
                ],
            ),
            self.period_section(
                format!("Week Since {}", recent.week.since.format(WEEK_START_FORMAT)),
                &recent.week,
                today,
            ),
            self.period_section(
                recent.month.since.format(MONTH_FORMAT).to_string(),
                &recent.month,
                today,
            ),
        ])
    }
}

impl ClaudeCcusagePeriods {
    fn working_days_text(&self) -> String {
        self.working_days
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn period_section(&self, title: String, period: &Period, today: NaiveDate) -> Section {
        let projected = period
            .projected_cost(today, &self.working_days)
            .map(|cost| KeyValue::new("Projected Cost", dollars(cost)));
        Section::dictionary(
            title,
            [
                Some(KeyValue::new("Cost", dollars(period.usage.cost))),
                projected,
                Some(KeyValue::new("Tokens", token_count(period.usage.tokens))),
            ]
            .into_iter()
            .flatten(),
        )
    }
}
