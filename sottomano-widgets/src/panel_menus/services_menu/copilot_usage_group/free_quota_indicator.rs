use sottomano_desktop_sources::{
    SnapshotSource,
    ai_usage_spend::{
        copilot_usage::{CopilotFree, QuotaReset, USER_ENDPOINT},
        subscription_plan::SubscriptionPlan,
    },
};

use super::{
    CopilotAccountSource, CopilotUsageBusRequest, CopilotUsageGroup, CopilotUsageInteraction,
};
use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_group_content::IndicatorListing,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::Quantity,
        localized_time::LocalizedTimeFormat,
        value_cells::{LabeledValue, ValueCell},
    },
};

pub(crate) const COPILOT_FREE_FALLBACK_GLYPH: IconGlyph =
    IconGlyph::Phosphor(Phosphor::ChatCircleDots(Weight::Regular));

pub fn copilot_free_indicator(free: &CopilotFree) -> GroupIndicator<'_> {
    let state = if free.chat <= 5 {
        IndicatorState::Critical
    } else if free.chat <= 20 {
        IndicatorState::Warning
    } else {
        IndicatorState::Idle
    };
    GroupIndicator::glyph_only(
        CopilotFree::NAME,
        Phosphor::ChatCircleDots(Weight::Regular).into(),
        state,
    )
    .with_readout(IndicatorReadout {
        label: ReadoutLabel::Labeled(format!("{} chats", free.chat)),
        gauge: ReadoutGauge::Ungauged,
        badge: ReadoutBadge::Unbadged,
    })
    .with_hover_card(Box::new(move |_| copilot_free_hover_card(free)))
}

pub fn copilot_free_hover_card(free: &CopilotFree) -> Vec<Section> {
    let mut subscription_rows = Vec::new();
    if let SubscriptionPlan::Named(plan) = &free.plan {
        subscription_rows.push(LabeledValue::new("Plan", plan.as_str()));
    }
    subscription_rows.push(LabeledValue::new("Source", USER_ENDPOINT));
    let mut reset_rows = Vec::new();
    if let QuotaReset::At(at) = free.reset {
        reset_rows.push(LabeledValue::new(
            "Resets",
            ValueCell::Time {
                at,
                format: LocalizedTimeFormat::Date,
            },
        ));
    }
    vec![
        Section::new("Subscription", dictionary_table(subscription_rows)),
        Section::new(
            "Remaining",
            dictionary_table(
                [
                    LabeledValue::new("Chat", Quantity::count(free.chat)),
                    LabeledValue::new("Completions", Quantity::count(free.completions)),
                ]
                .into_iter()
                .chain(reset_rows),
            ),
        ),
    ]
}

impl CopilotUsageGroup {
    pub(super) fn free_quota_listing(&self) -> IndicatorListing<'_> {
        self.free
            .indicator_listing(
                |held_snapshot| copilot_free_indicator(held_snapshot).when_shown(),
                COPILOT_FREE_FALLBACK_GLYPH,
            )
            .map_indicators(|row| self.free_indicator(row))
    }

    fn free_indicator<'view>(
        &'view self,
        free_row: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        free_row
            .with_hover_card(Box::new(move |_| {
                self.free.sections_described_by(copilot_free_hover_card)
            }))
            .refreshed_by(CopilotUsageInteraction::OnBuses(
                CopilotUsageBusRequest::Refresh(CopilotAccountSource::Free),
            ))
    }
}
