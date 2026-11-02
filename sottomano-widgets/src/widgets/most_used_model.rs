use chrono::Locale;
use rust_decimal::{Decimal, prelude::ToPrimitive};
use sottomano_sources::ai_usage_spend::{
    model_spend_ranking::{ModelRanking, ModelSpend},
    spend_money::{Money, usd},
};
use uom::si::{f64::Ratio, ratio::ratio};

use crate::{
    clock::Clock,
    widgets::{
        amount_text::{percent_text, spend_text},
        group_item_reading::GroupItemReading,
        icon_glyph::IconGlyph,
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Section, TooltipBody},
        value_cells::{LabeledValue, ValueCell},
        widget_state::WidgetState,
    },
};

const LISTED_MODELS: usize = 6;

#[derive(Debug, Clone, Copy)]
pub struct RankedSpan<'a> {
    pub heading: &'static str,
    pub models: &'a ModelRanking,
}

#[derive(Debug, Clone, Copy)]
pub struct ModelSpendRanking<'a, const SPANS: usize> {
    source: &'static str,
    narrowest_to_widest: [RankedSpan<'a>; SPANS],
}

impl<'a, const SPANS: usize> ModelSpendRanking<'a, SPANS> {
    #[must_use]
    pub fn new(source: &'static str, narrowest_to_widest: [RankedSpan<'a>; SPANS]) -> Option<Self> {
        let ranking = Self {
            source,
            narrowest_to_widest,
        };
        ranking.top()?;
        Some(ranking)
    }

    fn top(&self) -> Option<&ModelSpend> {
        self.narrowest_to_widest.last()?.models.top()
    }

    fn ranked_models(&self) -> Vec<&str> {
        self.narrowest_to_widest
            .iter()
            .rev()
            .flat_map(|span| span.models.iter().map(|spend| spend.model.as_str()))
            .fold(Vec::new(), |mut ranked, model| {
                if !ranked.contains(&model) {
                    ranked.push(model);
                }
                ranked
            })
    }

    fn tooltip(&self) -> TooltipBody {
        let ranked = self.ranked_models();
        let listed = &ranked[..ranked.len().min(LISTED_MODELS)];
        let other = TableRecord::new(
            "other",
            self.narrowest_to_widest.map(|span| {
                let unlisted = usd(span
                    .models
                    .iter()
                    .filter(|spend| !listed.contains(&spend.model.as_str()))
                    .map(|spend| *spend.cost.amount())
                    .sum());
                ValueCell::from(Quantity::spend(unlisted))
            }),
        );
        TooltipBody::sections([Section::column_table(
            self.narrowest_to_widest
                .map(|span| ColumnHeading::titled(span.heading)),
            listed
                .iter()
                .map(|model| {
                    TableRecord::new(
                        *model,
                        self.narrowest_to_widest.map(|span| {
                            Quantity::spend(
                                span.models
                                    .cost_of(model)
                                    .unwrap_or_else(|| usd(Decimal::ZERO)),
                            )
                            .into()
                        }),
                    )
                })
                .chain((ranked.len() > LISTED_MODELS).then_some(other)),
        )
        .titled(("Spend by Model", self.source))])
    }

    fn top_model_share(&self) -> TooltipBody {
        let top = self
            .top()
            .map(|spend| spend.model.clone())
            .unwrap_or_default();
        let span_shares = self.narrowest_to_widest.iter().filter_map(|span| {
            let cost = span.models.cost_of(&top).filter(Money::is_positive)?;
            let total = span.models.total();
            let share = (cost.amount() / total.amount()).to_f64()?;
            Some(LabeledValue::new(
                span.heading,
                format!(
                    "{cost} of {total}, {}",
                    percent_text(Ratio::new::<ratio>(share))
                ),
            ))
        });
        TooltipBody::sections([Section::dictionary(span_shares).titled((top, self.source))])
    }
}

pub struct MostUsedModel<'a, const SPANS: usize>(pub ModelSpendRanking<'a, SPANS>);

impl<const SPANS: usize> GroupItemReading for MostUsedModel<'_, SPANS> {
    fn name(&self) -> &'static str {
        "Most used model"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Sparkle(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(self.0.top()?.model.clone())
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        WidgetState::Idle
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        self.0.tooltip()
    }
}

pub struct MostUsedModelSpend<'a, const SPANS: usize>(pub ModelSpendRanking<'a, SPANS>);

impl<const SPANS: usize> GroupItemReading for MostUsedModelSpend<'_, SPANS> {
    fn name(&self) -> &'static str {
        "Most used model spend"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Coins(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(spend_text(&self.0.top()?.cost))
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        WidgetState::Idle
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        self.0.top_model_share()
    }
}
