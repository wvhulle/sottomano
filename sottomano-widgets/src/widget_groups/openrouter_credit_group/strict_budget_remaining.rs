use std::borrow::Cow;

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
        group_item_reading::{GroupItemReading, PanelVisibility},
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widgets::{
        amount_text::spend_text,
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

impl GroupItemReading for OpenRouterStrictBudget {
    fn name(&self) -> Cow<'static, str> {
        Self::NAME.into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Wallet(Weight::Regular).into()
    }

    fn fallback_icon() -> IconGlyph {
        Phosphor::Wallet(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(format!("{} left", spend_text(&self.tightest_remaining())))
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match self.credit_standing() {
            CreditStanding::Exhausted => ReadingDisplayState::Critical,
            CreditStanding::Remaining => ReadingDisplayState::Idle,
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections(
            [Section::dictionary([
                remaining_entry(self.credits.remaining()),
                LabeledValue::new("Lifetime Purchased", Quantity::spend(self.credits.total)),
            ])
            .titled("Credits")]
            .into_iter()
            .chain(self.key_limits.iter().map(Section::from)),
        )
    }

    fn panel_visibility(&self) -> PanelVisibility {
        if self.credits.total.is_positive() || !self.key_limits.is_empty() {
            PanelVisibility::Shown
        } else {
            PanelVisibility::Hidden
        }
    }
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
        Section::dictionary(
            [
                Some(LabeledValue::new("Limit", Quantity::spend(key_limit.limit))),
                Some(remaining_entry(key_limit.remaining)),
                resets,
            ]
            .into_iter()
            .flatten(),
        )
        .titled((key_limit.name.as_str(), "API key limit"))
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
