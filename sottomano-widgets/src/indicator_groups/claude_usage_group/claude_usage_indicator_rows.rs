use sottomano_desktop_sources::ai_usage_spend::claude_usage::SubscriptionCoverage;

use super::{ClaudeLocalSource, ClaudeUsageBusRequest, ClaudeUsageGroup, ClaudeUsageInteraction};
use crate::{
    clock::Clock,
    glyph_icons::{
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
    },
    group_indicators::group_indicator::{GroupIndicator, PanelVisibility},
    indicator_groups::{
        claude_usage_group::{
            claude_code_billing_block::{
                CLAUDE_CCUSAGE_BLOCK_FALLBACK_GLYPH, claude_ccusage_block_hover_card,
                claude_ccusage_block_indicator,
            },
            claude_code_period_spend::{
                CLAUDE_CCUSAGE_PERIODS_FALLBACK_GLYPH, claude_ccusage_periods_hover_card,
                claude_ccusage_periods_indicator,
            },
            claude_enterprise_credit_spend::{
                CLAUDE_ENTERPRISE_SPEND_FALLBACK_GLYPH, claude_enterprise_hover_card,
                claude_enterprise_indicator, claude_organization_sections,
            },
            claude_usage_window_gauge::{
                claude_usage_windows_hover_card, claude_usage_windows_indicator,
            },
        },
        indicator_group_content::IndicatorGroupContent,
        indicator_group_identity::IndicatorGroupFamily,
    },
    widgets::most_used_model::{ModelSpendRanking, RankedSpan},
};

impl ClaudeUsageGroup {
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

    fn enterprise_spend_indicator<'view>(
        &'view self,
        spend_row: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        spend_row
            .with_hover_card(Box::new(move |_| {
                let mut sections = self
                    .enterprise_spend
                    .sections_described_by(claude_enterprise_hover_card);
                sections.extend(
                    self.organization
                        .sections_described_by(claude_organization_sections),
                );
                sections
            }))
            .fetching_on_hover(
                ClaudeUsageInteraction::OnBuses(ClaudeUsageBusRequest::StartOrganizationFetch),
                ClaudeUsageInteraction::StopHoverFetches,
            )
            .refreshed_by(ClaudeUsageInteraction::OnBuses(
                ClaudeUsageBusRequest::RefreshEnterpriseSpend,
            ))
    }

    fn ccusage_periods_indicator<'view>(
        &'view self,
        periods_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        periods_indicator
            .with_hover_card(Box::new(move |card_clock| {
                self.ccusage_periods.sections_described_by(|snapshot| {
                    claude_ccusage_periods_hover_card(snapshot, card_clock)
                })
            }))
            .refreshed_by(ClaudeUsageInteraction::Refresh(
                ClaudeLocalSource::CcusagePeriods,
            ))
    }

    fn ccusage_block_indicator<'view>(
        &'view self,
        block_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        block_indicator
            .with_hover_card(Box::new(move |card_clock| {
                self.ccusage_block.sections_described_by(|snapshot| {
                    claude_ccusage_block_hover_card(snapshot, card_clock)
                })
            }))
            .refreshed_by(ClaudeUsageInteraction::Refresh(
                ClaudeLocalSource::CcusageBlock,
            ))
    }

    fn usage_windows_indicator<'view>(
        &'view self,
        windows_row: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        windows_row
            .with_hover_card(Box::new(move |card_clock| {
                self.usage_windows.sections_described_by(|snapshot| {
                    claude_usage_windows_hover_card(snapshot, card_clock)
                })
            }))
            .refreshed_by(ClaudeUsageInteraction::OnBuses(
                ClaudeUsageBusRequest::RefreshUsageWindows,
            ))
    }

    pub fn group_content(&self, clock: Clock) -> IndicatorGroupContent<'_> {
        let headline_indicator = self.usage_windows_headline(clock);
        let mut rows = Vec::new();
        self.push_enterprise_spend_indicators(&mut rows, clock);
        self.push_ccusage_periods_indicators(&mut rows, clock);
        self.push_ccusage_block_indicators(&mut rows);
        self.push_model_ranking_indicators(&mut rows);
        self.push_billed_ranking_indicators(&mut rows);
        self.list_usage_windows(clock, &mut rows);
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::ClaudeUsage,
            headline_indicator,
            &self.expansion,
            ClaudeUsageInteraction::Expansion,
            rows,
        )
    }

    fn usage_windows_headline(&self, clock: Clock) -> GroupIndicator<'_> {
        self.usage_windows_indicator(self.usage_windows.indicator(
            |held_snapshot| claude_usage_windows_indicator(held_snapshot, clock),
            Phosphor::Gauge(Weight::Regular).into(),
        ))
        .with_headline_glyph(SimpleIcon::Claude.into())
    }

    fn list_usage_windows<'view>(
        &'view self,
        clock: Clock,
        indicators: &mut Vec<GroupIndicator<'view>>,
    ) {
        if !self.usage_windows.awaits_first_delivery() {
            indicators.insert(
                0,
                self.usage_windows_indicator(self.usage_windows.indicator(
                    |held_snapshot| claude_usage_windows_indicator(held_snapshot, clock),
                    Phosphor::Gauge(Weight::Regular).into(),
                )),
            );
        }
    }

    fn push_enterprise_spend_indicators<'view>(
        &'view self,
        rows: &mut Vec<GroupIndicator<'view>>,
        clock: Clock,
    ) {
        if !self.enterprise_spend.awaits_first_delivery() {
            let spend_row = self.enterprise_spend.indicator(
                |held_snapshot| claude_enterprise_indicator(held_snapshot, clock),
                CLAUDE_ENTERPRISE_SPEND_FALLBACK_GLYPH,
            );
            if spend_row.panel_visibility == PanelVisibility::Shown {
                rows.push(self.enterprise_spend_indicator(spend_row));
            }
        }
    }

    fn push_ccusage_periods_indicators<'view>(
        &'view self,
        rows: &mut Vec<GroupIndicator<'view>>,
        clock: Clock,
    ) {
        if !self.ccusage_periods.awaits_first_delivery() {
            let periods_indicator = self.ccusage_periods.indicator(
                |held_snapshot| claude_ccusage_periods_indicator(held_snapshot, clock),
                CLAUDE_CCUSAGE_PERIODS_FALLBACK_GLYPH,
            );
            if periods_indicator.panel_visibility == PanelVisibility::Shown {
                rows.push(self.ccusage_periods_indicator(periods_indicator));
            }
        }
    }

    fn push_ccusage_block_indicators<'view>(&'view self, rows: &mut Vec<GroupIndicator<'view>>) {
        if !self.ccusage_block.awaits_first_delivery() {
            let block_indicator = self.ccusage_block.indicator(
                |held_snapshot| claude_ccusage_block_indicator(held_snapshot),
                CLAUDE_CCUSAGE_BLOCK_FALLBACK_GLYPH,
            );
            if block_indicator.panel_visibility == PanelVisibility::Shown {
                rows.push(self.ccusage_block_indicator(block_indicator));
            }
        }
    }

    fn push_model_ranking_indicators<'view>(&'view self, rows: &mut Vec<GroupIndicator<'view>>) {
        if let Some(ranking) = self.model_ranking() {
            rows.push(
                ranking
                    .most_used_model_indicator()
                    .with_hover_card(Box::new(move |_| {
                        self.ccusage_models.hover_card_with(ranking.hover_card())
                    }))
                    .refreshed_by(ClaudeUsageInteraction::Refresh(
                        ClaudeLocalSource::CcusageModels,
                    )),
            );
        }
    }

    fn push_billed_ranking_indicators<'view>(&'view self, rows: &mut Vec<GroupIndicator<'view>>) {
        if let Some(billed_ranking) = self.billed_model_ranking() {
            rows.push(
                billed_ranking
                    .most_used_spend_indicator()
                    .with_hover_card(Box::new(move |_| {
                        self.ccusage_models
                            .hover_card_with(billed_ranking.top_model_share())
                    }))
                    .refreshed_by(ClaudeUsageInteraction::Refresh(
                        ClaudeLocalSource::CcusageModels,
                    )),
            );
        }
    }
}
