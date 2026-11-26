use sottomano_desktop_sources::ai_usage_spend::claude_usage::SubscriptionCoverage;

use super::{ClaudeLocalSource, ClaudeUsageGroup, ClaudeUsageInteraction};
use crate::{
    group_indicators::group_indicator::GroupIndicator,
    widgets::most_used_model::{ModelSpendRanking, RankedSpan},
};

impl ClaudeUsageGroup {
    pub(super) fn most_used_model_rows(&self) -> Vec<GroupIndicator<'_>> {
        let model_rows = self.model_ranking().map(|ranking| {
            ranking
                .most_used_model_indicator()
                .with_hover_card(Box::new(move |_| {
                    self.ccusage_models.hover_card_with(ranking.hover_card())
                }))
                .refreshed_by(ClaudeUsageInteraction::Refresh(
                    ClaudeLocalSource::CcusageModels,
                ))
        });
        let billed_rows = self.billed_model_ranking().map(|billed_ranking| {
            billed_ranking
                .most_used_spend_indicator()
                .with_hover_card(Box::new(move |_| {
                    self.ccusage_models
                        .hover_card_with(billed_ranking.top_model_share())
                }))
                .refreshed_by(ClaudeUsageInteraction::Refresh(
                    ClaudeLocalSource::CcusageModels,
                ))
        });
        model_rows.into_iter().chain(billed_rows).collect()
    }

    fn billed_model_ranking(&self) -> Option<ModelSpendRanking<'_, 3>> {
        self.ccusage_models
            .get()
            .filter(|models| models.subscription_coverage == SubscriptionCoverage::Uncovered)
            .and(self.model_ranking())
    }

    fn model_ranking(&self) -> Option<ModelSpendRanking<'_, 3>> {
        let models = self.ccusage_models.get()?;
        ModelSpendRanking::new(
            "ccusage, local Claude Code logs, API list prices",
            [
                RankedSpan {
                    heading: "Today",
                    models: &models.today,
                },
                RankedSpan {
                    heading: "Week",
                    models: &models.week,
                },
                RankedSpan {
                    heading: "Month",
                    models: &models.month,
                },
            ],
        )
    }
}
