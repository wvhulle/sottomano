use chrono::Locale;
use sottomano_sources::ai_usage_spend::openrouter_model_rankings::{
    OpenRouterModelRankings, ScoredModel,
};
use uom::si::{f64::Ratio, ratio::ratio};

use crate::{
    clock::Clock,
    widgets::{
        amount_text::spend_text,
        group_item_reading::GroupItemReading,
        hover_card_body::{HoverCardBody, Section},
        icon_glyph::IconGlyph,
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        reading_display_state::ReadingDisplayState,
        value_cells::{LabeledValue, ValueCell},
    },
};

const LISTED_MODELS: usize = 16;
const ANALYSIS_LEADERBOARD_URL: &str = "https://artificialanalysis.ai/models";
const OPENROUTER_TOOL_MODELS_URL: &str = "https://openrouter.ai/models?supported_parameters=tools";

impl GroupItemReading for OpenRouterModelRankings {
    fn name(&self) -> &'static str {
        "Best coding models within price cap"
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
                ColumnHeading::titled("TB 4.0"),
                ColumnHeading::titled("TB 2.1"),
                ColumnHeading::titled("SciCode"),
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
                "Agentic Coding Models ≤ {} Blended",
                spend_text(&self.max_blended_per_million)
            ),
            "Terminal-Bench 4.0, then 2.1, then SciCode via Artificial Analysis; OpenRouter prices",
        ));
        let unscored_note = (self.unscored_count > 0).then(|| {
            Section::note(format!(
                "{} affordable models have no coding benchmark on Artificial Analysis",
                self.unscored_count
            ))
        });
        let source_links = Section::dictionary([
            LabeledValue::new(
                "Benchmarks",
                ValueCell::Link {
                    label: "Artificial Analysis".to_owned(),
                    url: ANALYSIS_LEADERBOARD_URL.to_owned(),
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

impl From<&ScoredModel> for TableRecord<6> {
    fn from(scored: &ScoredModel) -> Self {
        TableRecord::new(
            ValueCell::Link {
                label: scored.model.clone(),
                url: scored.openrouter_page(),
            },
            [
                benchmark_cell(scored.benchmarks.terminal_bench_v4),
                benchmark_cell(scored.benchmarks.terminal_bench_v2),
                benchmark_cell(scored.benchmarks.scicode),
                Quantity::spend(scored.input_per_million).into(),
                Quantity::spend(scored.output_per_million).into(),
                Quantity::compact_count(scored.context_tokens).into(),
            ],
        )
    }
}

fn benchmark_cell(score: Option<f64>) -> ValueCell {
    score.map_or(ValueCell::Blank, |score| {
        Quantity::share(Ratio::new::<ratio>(score)).into()
    })
}
