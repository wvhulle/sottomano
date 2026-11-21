use sottomano_desktop_sources::{
    SnapshotSource,
    ai_usage_spend::{
        openrouter_credits::{CreditStanding, KeyLimit, LimitReset, OpenRouterStrictBudget},
        spend_money::Money,
    },
};
use sottomano_toml_config::severity::Severity;

use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table},
    widgets::{amount_text::spend_text, localized_quantities::Quantity, value_cells::LabeledValue},
};

pub(crate) const OPEN_ROUTER_STRICT_BUDGET_FALLBACK_GLYPH: IconGlyph =
    IconGlyph::Phosphor(Phosphor::Wallet(Weight::Regular));

pub(crate) fn strict_budget_panel_visibility(budget: &OpenRouterStrictBudget) -> PanelVisibility {
    if budget.credits.total.is_positive() || !budget.key_limits.is_empty() {
        PanelVisibility::Shown
    } else {
        PanelVisibility::Hidden
    }
}

pub(crate) fn strict_budget_indicator(budget: &OpenRouterStrictBudget) -> GroupIndicator<'_> {
    let state = match budget.credit_standing() {
        CreditStanding::Exhausted => IndicatorState::Critical,
        CreditStanding::Remaining => IndicatorState::Idle,
    };
    GroupIndicator::glyph_only(
        OpenRouterStrictBudget::NAME,
        Phosphor::Wallet(Weight::Regular).into(),
        state,
    )
    .with_readout(IndicatorReadout {
        label: ReadoutLabel::Labeled(format!("{} left", spend_text(&budget.tightest_remaining()))),
        gauge: ReadoutGauge::Ungauged,
        badge: ReadoutBadge::Unbadged,
    })
    .with_panel_visibility(strict_budget_panel_visibility(budget))
    .with_hover_card(Box::new(move |_| strict_budget_hover_card(budget)))
}

pub(crate) fn strict_budget_hover_card(budget: &OpenRouterStrictBudget) -> Vec<Section> {
    [Section::new(
        "Credits",
        dictionary_table([
            remaining_entry(budget.credits.remaining()),
            LabeledValue::new("Lifetime Purchased", Quantity::spend(budget.credits.total)),
        ]),
    )]
    .into_iter()
    .chain(budget.key_limits.iter().map(Section::from))
    .collect::<Vec<_>>()
}

impl From<&KeyLimit> for Section {
    fn from(key_limit: &KeyLimit) -> Self {
        let mut limit_rows = vec![
            LabeledValue::new("Limit", Quantity::spend(key_limit.limit)),
            remaining_entry(key_limit.remaining),
        ];
        match key_limit.reset {
            LimitReset::Daily => limit_rows.push(LabeledValue::new("Resets", "daily")),
            LimitReset::Weekly => limit_rows.push(LabeledValue::new("Resets", "weekly")),
            LimitReset::Monthly => limit_rows.push(LabeledValue::new("Resets", "monthly")),
            LimitReset::Never => {}
        }
        Section::new(
            SectionTitle::captioned(key_limit.name.as_str(), "API key limit"),
            dictionary_table(limit_rows),
        )
    }
}

fn remaining_entry(remaining: Money) -> LabeledValue {
    let entry = LabeledValue::new("Remaining", Quantity::spend(remaining));
    if remaining.is_positive() {
        entry
    } else {
        entry.level(Severity::Critical)
    }
}
