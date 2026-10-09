use crate::{
    data::source::ai::openrouter::OpenRouter,
    widget_groups::ai_group::{amount_text::to_cents, spend_to_level::level_from_cost},
    widgets::{
        bar_widget::{BarWidget, Glyph, Occasional},
        simple_icons::SimpleIcon,
        tooltip_body::{Section, TooltipBody},
        widget_state::WidgetState,
    },
};

const LISTED_MODELS: usize = 6;

impl BarWidget for OpenRouter {
    fn icon(&self) -> Glyph {
        SimpleIcon::OpenRouter.into()
    }

    fn label(&self) -> Option<String> {
        Some(format!("${:.2}", self.spend))
    }

    fn state(&self) -> WidgetState {
        level_from_cost(self.spend).into()
    }

    fn tooltip(&self) -> TooltipBody {
        let remaining = self.credits.total - self.credits.used;
        TooltipBody::sections(&[
            Section::untitled(&[("Today", format!("${} USD", to_cents(self.spend)))]),
            Section::titled(
                "Credits",
                &[
                    ("Remaining", format!("${} USD", to_cents(remaining))),
                    (
                        "Lifetime Purchased",
                        format!("${} USD", to_cents(self.credits.total)),
                    ),
                ],
            ),
            model_section(&self.models),
        ])
    }
}

impl Occasional for OpenRouter {
    fn present(&self) -> bool {
        self.spend > 0.0 || self.overspent()
    }
}

fn model_section(models: &[(String, f64)]) -> Section {
    let rest: f64 = models
        .iter()
        .skip(LISTED_MODELS)
        .map(|(_, cost)| cost)
        .sum();
    let rows: Vec<(&str, Option<String>)> = models
        .iter()
        .take(LISTED_MODELS)
        .map(|(name, cost)| (name.as_str(), Some(format!("${}", to_cents(*cost)))))
        .chain([(
            "other",
            (rest > 0.0).then(|| format!("${}", to_cents(rest))),
        )])
        .collect();
    Section::titled("By Model Since Midnight", &rows)
}
