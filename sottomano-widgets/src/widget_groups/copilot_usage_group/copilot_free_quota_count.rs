use chrono::Locale;
use sottomano_sources::ai_usage_spend::copilot_usage::{CopilotFree, USER_ENDPOINT};

use crate::{
    clock::Clock,
    widgets::{
        group_item_reading::GroupItemReading,
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        reading_display_state::ReadingDisplayState,
        tooltip_body::{Section, TooltipBody},
        value_cells::{LabeledValue, ValueCell},
    },
};

impl GroupItemReading for CopilotFree {
    fn name(&self) -> &'static str {
        "Copilot free quota"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::ChatCircleDots(Weight::Regular).into()
    }

    fn fallback_icon() -> IconGlyph {
        Phosphor::ChatCircleDots(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(format!("{} chats", self.chat))
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        if self.chat <= 5 {
            ReadingDisplayState::Critical
        } else if self.chat <= 20 {
            ReadingDisplayState::Warning
        } else {
            ReadingDisplayState::Idle
        }
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([
            Section::dictionary(
                self.plan
                    .as_deref()
                    .map(|plan| LabeledValue::new("Plan", plan))
                    .into_iter()
                    .chain([LabeledValue::new("Source", USER_ENDPOINT)]),
            ),
            Section::dictionary(
                [
                    LabeledValue::new("Chat", Quantity::count(self.chat)),
                    LabeledValue::new("Completions", Quantity::count(self.completions)),
                ]
                .into_iter()
                .chain(self.reset.map(|at| {
                    LabeledValue::new(
                        "Resets",
                        ValueCell::Time {
                            at,
                            format: LocalizedTimeFormat::Date,
                        },
                    )
                })),
            )
            .titled("Remaining"),
        ])
    }
}
