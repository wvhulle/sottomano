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
        tooltip_body::{Section, TooltipBody, row, section, untitled},
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
        TooltipBody::Sections(vec![
            untitled(vec![
                row("Source", "ccusage, local Claude Code logs"),
                row("Cost Basis", "API list prices"),
                row("Working Days", self.working_days_text()),
                row("Projection", "cost per working day so far, today included"),
            ]),
            section(
                "Today",
                vec![
                    row("Cost", dollars(recent.today.usage.cost)),
                    row("Tokens", token_count(recent.today.usage.tokens)),
                    row("Models", recent.today.usage.models.clone()),
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
        let rows = [
            Some(row("Cost", dollars(period.usage.cost))),
            period
                .projected_cost(today, &self.working_days)
                .map(|projected| row("Projected Cost", dollars(projected))),
            Some(row("Tokens", token_count(period.usage.tokens))),
        ]
        .into_iter()
        .flatten()
        .collect();
        section(title, rows)
    }
}
