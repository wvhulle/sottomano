use vetrata_sources::{
    ai::openrouter::{OpenRouterSpend, SpanSpend},
    money::usd,
};

use crate::widgets::{
    amount_text::spend_text,
    bar_widget::{BarWidget, Glyph},
    phosphor::Phosphor,
    tooltip_body::{Cell, ColumnHeading, Section, TableRecord, TooltipBody},
    widget_state::WidgetState,
};

const LISTED_MODELS: usize = 6;

pub struct ModelSpendBreakdown<'a>(pub &'a OpenRouterSpend);

impl BarWidget for ModelSpendBreakdown<'_> {
    fn name(&self) -> &'static str {
        "OpenRouter spend by model"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Sparkle(self.state().weight()).into()
    }

    fn label(&self) -> Option<String> {
        let top = self.0.month.models.first()?;
        Some(format!("{} {}", top.model, spend_text(&top.cost)))
    }

    fn state(&self) -> WidgetState {
        WidgetState::Idle
    }

    fn tooltip(&self) -> TooltipBody {
        let ranked = self.ranked_models();
        let listed = &ranked[..ranked.len().min(LISTED_MODELS)];
        let other = TableRecord::new(
            "other",
            self.spans().map(|span| {
                let unlisted = usd(span
                    .models
                    .iter()
                    .filter(|spend| !listed.contains(&spend.model.as_str()))
                    .map(|spend| *spend.cost.amount())
                    .sum());
                unlisted
                    .is_positive()
                    .then(|| Cell::from(unlisted.to_string()))
            }),
        );
        TooltipBody::sections([Section::column_table(
            [
                ColumnHeading::new("Today"),
                ColumnHeading::new("Week"),
                ColumnHeading::new("Month"),
            ],
            listed
                .iter()
                .map(|model| {
                    TableRecord::new(
                        *model,
                        self.spans()
                            .map(|span| span.model_cost(model).map(|cost| cost.to_string().into())),
                    )
                })
                .chain((ranked.len() > LISTED_MODELS).then_some(other)),
        )
        .titled((
            "Spend by Model",
            "OpenRouter analytics, per local calendar span",
        ))])
    }
}

impl ModelSpendBreakdown<'_> {
    fn spans(&self) -> [&SpanSpend; 3] {
        [&self.0.today, &self.0.week, &self.0.month]
    }

    fn ranked_models(&self) -> Vec<&str> {
        [&self.0.month, &self.0.week, &self.0.today]
            .into_iter()
            .flat_map(|span| span.models.iter().map(|spend| spend.model.as_str()))
            .fold(Vec::new(), |mut ranked, model| {
                if !ranked.contains(&model) {
                    ranked.push(model);
                }
                ranked
            })
    }
}
