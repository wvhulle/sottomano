use chrono::Locale;
use sottomano_desktop_sources::ai_usage_spend::openrouter_model_rankings::{
    OpenRouterModelRankings, ScoredModel,
};

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading, reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widgets::{
        amount_text::spend_text,
        icon_glyph::IconGlyph,
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_quantities::{DisplayUnit, Quantity},
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
};

const LISTED_MODELS: usize = 16;
const INTELLIGENCE_INDEX_DECIMALS: u8 = 1;
const OPENROUTER_RANKINGS_URL: &str = "https://openrouter.ai/rankings";
const OPENROUTER_TOOL_MODELS_URL: &str = "https://openrouter.ai/models?supported_parameters=tools";

impl GroupItemReading for OpenRouterModelRankings {
    fn name(&self) -> &'static str {
        "Best models within price cap"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Ranking(Weight::Regular).into()
    }

    fn fallback_icon() -> IconGlyph {
        Phosphor::Ranking(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(self.best_first.first()?.model.clone())
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::Idle
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let ranking_table = Section::column_table(
            [
                ColumnHeading::titled("Intelligence"),
                ColumnHeading::titled("In/Mtok"),
                ColumnHeading::titled("Out/Mtok"),
                ColumnHeading::titled("Context"),
            ],
            self.best_first
                .iter()
                .take(LISTED_MODELS)
                .map(TableRecord::from),
        )
        .titled((
            format!(
                "Tool-Capable Models ≤ {} Blended",
                spend_text(&self.max_blended_per_million)
            ),
            "Intelligence Index from Artificial Analysis via OpenRouter; OpenRouter prices",
        ));
        let unscored_note = (self.unscored_count > 0).then(|| {
            Section::note(format!(
                "{} affordable models have no Intelligence Index on OpenRouter",
                self.unscored_count
            ))
        });
        let source_links = Section::dictionary([
            LabeledValue::new(
                "Benchmarks",
                ValueCell::Link {
                    label: "OpenRouter rankings".to_owned(),
                    url: OPENROUTER_RANKINGS_URL.to_owned(),
                },
            ),
            LabeledValue::new(
                "Prices",
                ValueCell::Link {
                    label: "OpenRouter models".to_owned(),
                    url: OPENROUTER_TOOL_MODELS_URL.to_owned(),
                },
            ),
        ])
        .titled("Sources");
        HoverCardBody::sections(
            [ranking_table]
                .into_iter()
                .chain(unscored_note)
                .chain([source_links]),
        )
    }

    fn visible_on_panel(&self) -> bool {
        !self.best_first.is_empty()
    }
}

impl From<&ScoredModel> for TableRecord<4> {
    fn from(scored: &ScoredModel) -> Self {
        TableRecord::new(
            ValueCell::Link {
                label: scored.model.clone(),
                url: scored.openrouter_page(),
            },
            [
                Quantity::measured(
                    scored.intelligence_index,
                    DisplayUnit::Unitless,
                    INTELLIGENCE_INDEX_DECIMALS,
                )
                .into(),
                Quantity::spend(scored.input_per_million).into(),
                Quantity::spend(scored.output_per_million).into(),
                Quantity::compact_count(scored.context_tokens).into(),
            ],
        )
    }
}
