use super::{
    OpenRouterCreditGroup, OpenRouterCreditSource, credit_refresh_interaction,
    spend_indicator::spend_panel_visibility,
};
use crate::{
    group_indicators::group_indicator::GroupIndicator,
    widgets::most_used_model::{ModelSpendRanking, RankedSpan},
};

impl OpenRouterCreditGroup {
    pub(super) fn most_used_model_rows(&self) -> Vec<GroupIndicator<'_>> {
        self.model_ranking()
            .map(|ranking| {
                [
                    ranking
                        .most_used_model_indicator()
                        .with_hover_card(Box::new(move |_| {
                            self.spend.hover_card_with(ranking.hover_card())
                        }))
                        .refreshed_by(credit_refresh_interaction(OpenRouterCreditSource::Spend)),
                    ranking
                        .most_used_spend_indicator()
                        .with_hover_card(Box::new(move |_| {
                            self.spend.hover_card_with(ranking.top_model_share())
                        }))
                        .refreshed_by(credit_refresh_interaction(OpenRouterCreditSource::Spend)),
                ]
            })
            .into_iter()
            .flatten()
            .collect()
    }

    fn model_ranking(&self) -> Option<ModelSpendRanking<'_, 3>> {
        let spend = self.spend.visible_indicator(spend_panel_visibility)?;
        ModelSpendRanking::new(
            "OpenRouter analytics, per local calendar span",
            [
                RankedSpan {
                    heading: "Today",
                    models: &spend.today.models,
                },
                RankedSpan {
                    heading: "Week",
                    models: &spend.week.models,
                },
                RankedSpan {
                    heading: "Month",
                    models: &spend.month.models,
                },
            ],
        )
    }
}
