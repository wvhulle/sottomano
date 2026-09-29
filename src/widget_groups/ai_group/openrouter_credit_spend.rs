use crate::{
    data::source::ai::openrouter::OpenRouter,
    widget_groups::ai_group::{amount_text::to_cents, spend_to_level::level_from_cost},
    widgets::{
        bar_widget::{BarWidget, Glyph, Occasional},
        simple_icons::SimpleIcon,
        tooltip_body::{Section, TooltipBody, row, section, untitled},
        widget_state::WidgetState,
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

    fn state(&self) -> WidgetState {
        level_from_cost(self.spend).into()
    }

    fn tooltip(&self) -> TooltipBody {
        let remaining = self.credits.total - self.credits.used;
        let mut sections = vec![
            untitled(vec![row("Today", format!("${} USD", to_cents(self.spend)))]),
            section(
                "Credits",
                vec![
                    row("Remaining", format!("${} USD", to_cents(remaining))),
                    row(
                        "Lifetime Purchased",
                        format!("${} USD", to_cents(self.credits.total)),
                    ),
                ],
            ),
        ];
        sections.extend(model_section(&self.models));
        TooltipBody::Sections(sections)
    }
}

impl Occasional for OpenRouter {
    fn present(&self) -> bool {
        self.spend > 0.0 || self.overspent()
    }
}

fn model_section(models: &[(String, f64)]) -> Option<Section> {
    if models.is_empty() {
        return None;
    }
    let mut rows: Vec<_> = models
        .iter()
        .take(LISTED_MODELS)
        .map(|(name, cost)| row(name, format!("${}", to_cents(*cost))))
        .collect();
    let rest: f64 = models
        .iter()
        .skip(LISTED_MODELS)
        .map(|(_, cost)| cost)
        .sum();
    if rest > 0.0 {
        rows.push(row("other", format!("${}", to_cents(rest))));
    }
    Some(section("By Model Since Midnight", rows))
}
