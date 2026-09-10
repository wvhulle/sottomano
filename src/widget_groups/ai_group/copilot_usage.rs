use crate::{
    data::source::ai::copilot::{CopilotCcusage, CopilotFree, CopilotMetered},
    widget_groups::ai_group::spend_to_level::{level_from_cost, level_from_used},
    widgets::{
        bar_widget::{BarWidget, Glyph, Level, Occasional, Unit, Value},
        simple_icons::SimpleIcon,
        tooltip_body::{Row, TooltipBody, row},
    },
};

const USER_ENDPOINT: &str = "/copilot_internal/user";

impl BarWidget for CopilotMetered {
    fn icon(&self) -> Glyph {
        SimpleIcon::GithubCopilot.into()
    }

    fn level(&self) -> Level {
        level_from_used(self.percent_used)
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Gauge {
            percent: self.percent_used,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        let mut rows = vec![
            row("Plan", self.plan.clone()),
            row("Source", USER_ENDPOINT),
            row("Quota", self.quota.clone()),
            row("Used", self.used.to_string()),
            row("Limit", self.limit.to_string()),
            row("Remaining", self.remaining.to_string()),
        ];
        rows.extend(reset_row(self.reset.as_ref()));
        TooltipBody::rows(rows)
    }
}

impl Occasional for CopilotMetered {
    fn present(&self) -> bool {
        self.percent_used > 0
    }
}

impl BarWidget for CopilotFree {
    fn icon(&self) -> Glyph {
        SimpleIcon::GithubCopilot.into()
    }

    fn label(&self) -> Option<String> {
        Some(self.chat.to_string())
    }

    fn level(&self) -> Level {
        if self.chat <= 5 {
            Level::Critical
        } else if self.chat <= 20 {
            Level::Warning
        } else {
            Level::Ok
        }
    }

    fn tooltip(&self) -> TooltipBody {
        let mut rows = vec![
            row("Plan", self.plan.clone()),
            row("Source", USER_ENDPOINT),
            row("Chat", self.chat.to_string()),
            row("Completions", self.completions.to_string()),
        ];
        rows.extend(reset_row(self.reset.as_ref()));
        TooltipBody::rows(rows)
    }
}

impl BarWidget for CopilotCcusage {
    fn icon(&self) -> Glyph {
        SimpleIcon::GithubCopilot.into()
    }

    fn label(&self) -> Option<String> {
        Some(if self.cost > 0.0 {
            format!("${:.2}", self.cost)
        } else if self.tokens > 0 {
            "cli".to_owned()
        } else {
            String::new()
        })
    }

    fn level(&self) -> Level {
        level_from_cost(self.cost)
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::rows(vec![
            row("Plan", "copilot-cli"),
            row("Source", "ccusage"),
            row("Scope", "local CLI session data"),
            row("Month", self.month.clone()),
            row("MonthTokens", self.tokens.to_string()),
            row("MonthCost", format!("${:.2}", self.cost)),
            row("Models", self.models.clone()),
        ])
    }
}

impl Occasional for CopilotCcusage {
    fn present(&self) -> bool {
        self.tokens > 0 || self.cost > 0.0
    }
}

fn reset_row(reset: Option<&String>) -> Option<Row> {
    reset.map(|reset| row("Resets", reset.clone()))
}
