use uom::si::f64::Ratio;

use crate::{
    data::source::ai::claude::{ClaudeCcusage, ClaudeOauth, ExtraUsage, WindowUsage},
    widget_groups::ai_group::spend_to_level::{level_from_cost, level_from_used},
    widgets::{
        bar_widget::{BarWidget, Glyph, Occasional},
        gauge::percent_fill,
        simple_icons::SimpleIcon,
        tooltip_body::{Row, TooltipBody, row},
        widget_state::WidgetState,
    },
};

impl BarWidget for ClaudeCcusage {
    fn icon(&self) -> Glyph {
        SimpleIcon::Claude.into()
    }

    fn state(&self) -> WidgetState {
        level_from_cost(self.cost).into()
    }

    fn tooltip(&self) -> TooltipBody {
        if self.tokens == 0 && self.cost == 0.0 {
            return TooltipBody::rows(vec![row("Usage", "no activity today")]);
        }
        TooltipBody::rows(vec![
            row("Today", format!("${:.2} USD", self.cost)),
            row("Tokens", self.tokens.to_string()),
            row("Models", self.models.clone()),
        ])
    }
}

impl BarWidget for ClaudeOauth {
    fn icon(&self) -> Glyph {
        SimpleIcon::Claude.into()
    }

    fn state(&self) -> WidgetState {
        level_from_used(self.session_used()).into()
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(percent_fill(self.session_used()))
    }

    fn tooltip(&self) -> TooltipBody {
        let mut rows = vec![row("Plan", self.plan.clone())];
        rows.extend(window_rows("Session", &self.session));
        rows.extend(window_rows("Weekly", &self.weekly));
        if let Some(sonnet) = &self.sonnet {
            rows.extend(window_rows("Sonnet", sonnet));
        }
        if let Some(extra) = &self.extra {
            rows.extend(extra_rows(extra));
        }
        TooltipBody::rows(rows)
    }
}

impl Occasional for ClaudeOauth {
    fn present(&self) -> bool {
        self.session_used() > 0
    }
}

fn window_rows(label: &str, window: &WindowUsage) -> Vec<Row> {
    vec![
        row(label, format!("{}%", window.used)),
        row(format!("{label} resets"), window.resets_in.clone()),
    ]
}

fn extra_rows(extra: &ExtraUsage) -> Vec<Row> {
    [
        extra
            .spent_credits
            .map(|spent| row("Extra spent", format!("€{:.2}", spent / 100.0))),
        extra
            .monthly_limit
            .map(|limit| row("Extra limit", format!("€{:.0}", limit / 100.0))),
        extra.used.map(|used| row("Extra", format!("{used}%"))),
    ]
    .into_iter()
    .flatten()
    .collect()
}
