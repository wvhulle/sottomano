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
        tooltip_body::{Section, TooltipBody},
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
            Section::untitled(&[
                ("Source", "ccusage, local Claude Code logs"),
                ("Cost Basis", "API list prices"),
                ("Working Days", &self.working_days_text()),
                ("Projection", "cost per working day so far, today included"),
            ]),
            Section::titled(
                "Today",
                &[
                    ("Cost", dollars(recent.today.usage.cost)),
                    ("Tokens", token_count(recent.today.usage.tokens)),
                    ("Models", recent.today.usage.models.clone()),
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
        Section::titled(
            title,
            &[
                ("Cost", Some(dollars(period.usage.cost))),
                (
                    "Projected Cost",
                    period
                        .projected_cost(today, &self.working_days)
                        .map(dollars),
                ),
                ("Tokens", Some(token_count(period.usage.tokens))),
            ],
        )
    }
}
