use chrono::Locale;
use sottomano_desktop_sources::{
    SnapshotSource,
    ai_usage_spend::{
        openrouter_credits::{CreditStanding, KeyLimit, LimitReset, OpenRouterStrictBudget},
        spend_money::Money,
    },
};
use sottomano_toml_config::severity::Severity;

use crate::{
    clock::Clock,
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::{PanelVisibility, ReadingRow},
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table},
    widgets::{
        amount_text::spend_text,
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
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

pub(crate) fn strict_budget_reading_row(
    budget: &OpenRouterStrictBudget,
    _clock: Clock,
    _locale: Locale,
) -> ReadingRow<'_> {
    let state = match budget.credit_standing() {
        CreditStanding::Exhausted => ReadingDisplayState::Critical,
        CreditStanding::Remaining => ReadingDisplayState::Idle,
    };
    ReadingRow {
        readout: RowReadout {
            label: ReadoutLabel::Labeled(format!(
                "{} left",
                spend_text(&budget.tightest_remaining())
            )),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        },
        panel_visibility: strict_budget_panel_visibility(budget),
        ..ReadingRow::glyph_only(
            OpenRouterStrictBudget::NAME,
            Phosphor::Wallet(Weight::Regular).into(),
            state,
            Box::new(move |card_clock| strict_budget_hover_card(budget, card_clock)),
        )
    }
}

pub(crate) fn strict_budget_hover_card(
    budget: &OpenRouterStrictBudget,
    _clock: Clock,
) -> Vec<Section> {
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
        let resets = key_limit.reset.map(|reset| {
            let period_name = match reset {
                LimitReset::Daily => "daily",
                LimitReset::Weekly => "weekly",
                LimitReset::Monthly => "monthly",
            };
            LabeledValue::new("Resets", period_name)
        });
        Section::new(
            SectionTitle::captioned(key_limit.name.as_str(), "API key limit"),
            dictionary_table(
                [
                    Some(LabeledValue::new("Limit", Quantity::spend(key_limit.limit))),
                    Some(remaining_entry(key_limit.remaining)),
                    resets,
                ]
                .into_iter()
                .flatten(),
            ),
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
