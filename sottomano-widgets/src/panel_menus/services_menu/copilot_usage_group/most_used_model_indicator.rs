use super::{
    CopilotUsageGroup, CopilotUsageInteraction,
    cli_spend_indicator::copilot_ccusage_panel_visibility,
};
use crate::{
    group_indicators::group_indicator::GroupIndicator,
    widgets::most_used_model::{ModelSpendRanking, RankedSpan},
};

impl CopilotUsageGroup {
    pub(super) fn most_used_model_rows(&self) -> Vec<GroupIndicator<'_>> {
        self.model_ranking()
            .map(|ranking| {
                [
                    ranking
                        .most_used_model_indicator()
                        .with_hover_card(Box::new(move |_| {
                            self.cli.hover_card_with(ranking.hover_card())
                        }))
                        .refreshed_by(CopilotUsageInteraction::RefreshCli),
                    ranking
                        .most_used_spend_indicator()
                        .with_hover_card(Box::new(move |_| {
                            self.cli.hover_card_with(ranking.top_model_share())
                        }))
                        .refreshed_by(CopilotUsageInteraction::RefreshCli),
                ]
            })
            .into_iter()
            .flatten()
            .collect()
    }

    fn model_ranking(&self) -> Option<ModelSpendRanking<'_, 1>> {
        let cli = self
            .cli
            .visible_indicator(copilot_ccusage_panel_visibility)?;
        ModelSpendRanking::new(
            "ccusage, local Copilot CLI session data",
            [RankedSpan {
                heading: "Month",
                models: &cli.period.usage.models,
            }],
        )
    }
}
