use chrono::Locale;
use sottomano_sources::ai_usage_spend::openrouter_model_rankings::{
    OpenRouterModelRankings, ScoredModel,
};
use uom::si::{f64::Ratio, ratio::ratio};

use crate::{
    clock::Clock,
    widgets::{
        amount_text::{percent_text, spend_text},
        bar_widget::{BarWidget, Glyph},
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Section, TooltipBody},
        value_cells::{LabeledValue, ValueCell},
        widget_state::WidgetState,
    },
};

const LISTED_MODELS: usize = 16;
const ANALYSIS_LEADERBOARD_URL: &str = "https://artificialanalysis.ai/models";
const OPENROUTER_TOOL_MODELS_URL: &str = "https://openrouter.ai/models?supported_parameters=tools";

impl BarWidget for OpenRouterModelRankings {
    fn name(&self) -> &'static str {
        "Best coding models within price cap"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Ranking(Weight::Regular).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::Ranking(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(self.best_first.first()?.model.clone())
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        WidgetState::Idle
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
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
        TooltipBody::sections(
            [ranking_table]
                .into_iter()
                .chain(unscored_note)
                .chain([source_links]),
        )
    }

    fn visible_in_bar(&self) -> bool {
        !self.best_first.is_empty()
    }
}

impl From<&ScoredModel> for TableRecord<6> {
    fn from(scored: &ScoredModel) -> Self {
        let analysis_page = scored.analysis_page();
        TableRecord::new(
            ValueCell::Link {
                label: scored.model.clone(),
                url: scored.openrouter_page(),
            },
            [
                benchmark_cell(scored.benchmarks.terminal_bench_v4, &analysis_page),
                benchmark_cell(scored.benchmarks.terminal_bench_v2, &analysis_page),
                benchmark_cell(scored.benchmarks.scicode, &analysis_page),
                Quantity::spend(scored.input_per_million).into(),
                Quantity::spend(scored.output_per_million).into(),
                Quantity::compact_count(scored.context_tokens).into(),
            ],
        )
    }
}

fn benchmark_cell(score: Option<f64>, analysis_page: &str) -> ValueCell {
    score.map_or(ValueCell::Blank, |score| ValueCell::Link {
        label: percent_text(Ratio::new::<ratio>(score)),
        url: analysis_page.to_owned(),
    })
}
