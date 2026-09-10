use crate::{
    data::source::ai::openrouter::OpenRouter,
    widget_groups::ai_group::spend_to_level::level_from_cost,
    widgets::{
        bar_widget::{BarWidget, Glyph, Level, Occasional},
        simple_icons::SimpleIcon,
        tooltip_body::{Row, TooltipBody, row},
    },
};

const LISTED_MODELS: usize = 6;

impl BarWidget for OpenRouter {
    fn icon(&self) -> Glyph {
        SimpleIcon::OpenRouter.into()
    }

    fn label(&self) -> Option<String> {
        Some(format!("{:.2}", self.spend))
    }

    fn level(&self) -> Level {
        level_from_cost(self.spend)
    }

    fn tooltip(&self) -> TooltipBody {
        let remaining = self.credits.total - self.credits.used;
        let mut rows = vec![
            row("Today", format!("${} USD", money(self.spend))),
            row("Credits remaining", format!("${} USD", money(remaining))),
            row(
                "Lifetime credits purchased",
                format!("${} USD", money(self.credits.total)),
            ),
        ];
        rows.extend(model_rows(&self.models));
        TooltipBody::rows(rows)
    }
}

impl Occasional for OpenRouter {
    fn present(&self) -> bool {
        self.spend > 0.0 || self.overspent()
    }
}

fn model_rows(models: &[(String, f64)]) -> Vec<Row> {
    if models.is_empty() {
        return Vec::new();
    }
    let mut rows = vec![row(" ", "by model (since midnight)")];
    rows.extend(
        models
            .iter()
            .take(LISTED_MODELS)
            .map(|(name, cost)| row(name, format!("${}", money(*cost)))),
    );
    let rest: f64 = models
        .iter()
        .skip(LISTED_MODELS)
        .map(|(_, cost)| cost)
        .sum();
    if rest > 0.0 {
        rows.push(row("other", format!("${}", money(rest))));
    }
    rows
}

fn money(value: f64) -> String {
    format!("{}", (value * 100.0).round() / 100.0)
}
