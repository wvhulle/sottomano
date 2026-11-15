use std::num::NonZeroUsize;

use chrono::Locale;
use sottomano_desktop_sources::{
    SnapshotSource,
    ai_usage_spend::openrouter_model_rankings::{OpenRouterModelRankings, ScoredModel},
};

use crate::{
    clock::Clock,
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::{PanelVisibility, ReadingRow},
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{
        Section, SectionTitle, SectionWidget, dictionary_table, headed_table,
    },
    widgets::{
        amount_text::spend_text,
        icon_glyph::IconGlyph,
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_quantities::{DisplayUnit, Quantity},
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
};

const LISTED_MODELS: NonZeroUsize = NonZeroUsize::new(16).unwrap();
const INTELLIGENCE_INDEX_DECIMALS: u8 = 1;
const OPENROUTER_RANKINGS_URL: &str = "https://openrouter.ai/rankings";
const OPENROUTER_TOOL_MODELS_URL: &str = "https://openrouter.ai/models?supported_parameters=tools";

pub(crate) const OPEN_ROUTER_MODEL_RANKINGS_FALLBACK_GLYPH: IconGlyph =
    IconGlyph::Phosphor(Phosphor::Ranking(Weight::Regular));

pub(crate) fn best_models_panel_visibility(rankings: &OpenRouterModelRankings) -> PanelVisibility {
    if rankings.best_first.is_empty() {
        PanelVisibility::Hidden
    } else {
        PanelVisibility::Shown
    }
}

pub(crate) fn best_models_reading_row(
    rankings: &OpenRouterModelRankings,
    _clock: Clock,
    _locale: Locale,
) -> ReadingRow<'_> {
    let label = match rankings.best_first.first() {
        Some(scored) => ReadoutLabel::Labeled(scored.model.clone()),
        None => ReadoutLabel::Unlabeled,
    };
    ReadingRow {
        readout: RowReadout {
            label,
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        },
        panel_visibility: best_models_panel_visibility(rankings),
        ..ReadingRow::glyph_only(
            OpenRouterModelRankings::NAME,
            Phosphor::Ranking(Weight::Regular).into(),
            ReadingDisplayState::Idle,
            Box::new(move |card_clock| best_models_hover_card(rankings, card_clock)),
        )
    }
}

pub(crate) fn best_models_hover_card(
    rankings: &OpenRouterModelRankings,
    _clock: Clock,
) -> Vec<Section> {
    let ranking_table = Section::new(
        SectionTitle::captioned(
            format!(
                "Tool-Capable Models ≤ {} Blended",
                spend_text(&rankings.max_blended_per_million)
            ),
            "Intelligence Index from Artificial Analysis via OpenRouter; OpenRouter prices",
        ),
        headed_table(
            [
                ColumnHeading::titled("Intelligence"),
                ColumnHeading::titled("In/Mtok"),
                ColumnHeading::titled("Out/Mtok"),
                ColumnHeading::titled("Context"),
                ColumnHeading::titled("Page"),
            ],
            rankings.best_first.iter().map(TableRecord::from),
        )
        .truncated_to(LISTED_MODELS),
    );
    let unscored_note = (rankings.unscored_count > 0).then(|| {
        Section::new(
            "Unscored Models",
            SectionWidget::paragraph(format!(
                "{} affordable models have no Intelligence Index on OpenRouter",
                rankings.unscored_count
            )),
        )
    });
    let source_links = Section::new(
        "Sources",
        dictionary_table([
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
        ]),
    );
    [ranking_table]
        .into_iter()
        .chain(unscored_note)
        .chain([source_links])
        .collect::<Vec<_>>()
}

impl From<&ScoredModel> for TableRecord<5> {
    fn from(scored: &ScoredModel) -> Self {
        TableRecord::with_cells(
            ValueCell::Text(scored.model.clone()),
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
                ValueCell::Link {
                    label: "↗".to_owned(),
                    url: scored.openrouter_page(),
                },
            ],
        )
    }
}
