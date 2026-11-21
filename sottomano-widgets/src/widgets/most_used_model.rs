use rust_decimal::{Decimal, prelude::ToPrimitive};
use sottomano_desktop_sources::ai_usage_spend::{
    model_spend_ranking::{ModelRanking, ModelSpend},
    spend_money::{Money, usd},
};
use uom::si::{f64::Ratio, ratio::ratio};

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table, headed_table},
    widgets::{
        amount_text::{percent_text, spend_text},
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_quantities::Quantity,
        value_cells::{LabeledValue, ValueCell},
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
        self.narrowest_to_widest.last()?.models.model_spends.first()
    }

    fn ranked_models(&self) -> Vec<&str> {
        self.narrowest_to_widest
            .iter()
            .rev()
            .flat_map(|span| {
                span.models
                    .model_spends
                    .iter()
                    .map(|spend| spend.model.as_str())
            })
            .fold(Vec::new(), |mut ranked, model| {
                if !ranked.contains(&model) {
                    ranked.push(model);
                }
                ranked
            })
    }

    #[must_use]
    pub fn hover_card(&self) -> Vec<Section> {
        let ranked = self.ranked_models();
        let listed = &ranked[..ranked.len().min(LISTED_MODELS)];
        let other = TableRecord::with_cells(
            "other",
            self.narrowest_to_widest.map(|span| {
                let unlisted = usd(span
                    .models
                    .model_spends
                    .iter()
                    .filter(|spend| !listed.contains(&spend.model.as_str()))
                    .map(|spend| *spend.cost.amount())
                    .sum());
                ValueCell::from(Quantity::spend(unlisted))
            }),
        );
        vec![Section::new(
            SectionTitle::captioned("Spend by Model", self.source),
            headed_table(
                self.narrowest_to_widest
                    .map(|span| ColumnHeading::titled(span.heading)),
                listed
                    .iter()
                    .map(|model| {
                        TableRecord::with_cells(
                            *model,
                            self.narrowest_to_widest.map(|span| {
                                Quantity::spend(
                                    span.models
                                        .model_cost(model)
                                        .unwrap_or_else(|| usd(Decimal::ZERO)),
                                )
                                .into()
                            }),
                        )
                    })
                    .chain((ranked.len() > LISTED_MODELS).then_some(other)),
            ),
        )]
    }

    #[must_use]
    pub fn top_model_share(&self) -> Vec<Section> {
        let top = self
            .top()
            .map(|spend| spend.model.clone())
            .unwrap_or_default();
        let span_shares = self.narrowest_to_widest.iter().filter_map(|span| {
            let cost = span.models.model_cost(&top).filter(Money::is_positive)?;
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
        vec![Section::new(
            SectionTitle::captioned(top.clone(), self.source),
            dictionary_table(span_shares),
        )]
    }

    #[must_use]
    pub fn most_used_model_indicator(self) -> GroupIndicator<'a> {
        let label = match self.top() {
            Some(spend) => ReadoutLabel::Labeled(spend.model.clone()),
            None => ReadoutLabel::Unlabeled,
        };
        GroupIndicator::glyph_only(
            "Most used model",
            Phosphor::Sparkle(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_readout(IndicatorReadout {
            label,
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    #[must_use]
    pub fn most_used_spend_indicator(self) -> GroupIndicator<'a> {
        let label = match self.top() {
            Some(spend) => ReadoutLabel::Labeled(spend_text(&spend.cost)),
            None => ReadoutLabel::Unlabeled,
        };
        GroupIndicator::glyph_only(
            "Most used model spend",
            Phosphor::Coins(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_readout(IndicatorReadout {
            label,
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.top_model_share()))
    }
}
