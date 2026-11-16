use sottomano_desktop_sources::wallpaper_place_article::WikipediaArticle;

use crate::{
    group_indicators::{
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
        reading_row::ReadingRow,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        simple_icons::SimpleIcon,
        value_cells::{LabeledValue, ValueCell},
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct Article {
    title: String,
    url: String,
    summary: Option<String>,
}

impl Article {
    pub(super) fn reading_row(&self) -> ReadingRow<'_> {
        ReadingRow {
            readout: IndicatorReadout {
                label: match Some(&self.title) {
                    Some(text) if !text.is_empty() => ReadoutLabel::Labeled(text.clone()),
                    Some(_) | None => ReadoutLabel::Unlabeled,
                },
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                "Wallpaper article",
                SimpleIcon::Wikipedia.into(),
                IndicatorState::Idle,
                Box::new(move |_| self.hover_card()),
            )
        }
    }

    pub(super) fn hover_card(&self) -> Vec<Section> {
        vec![
            Section::new(
                "Wikipedia",
                dictionary_table([LabeledValue::new(
                    "Article",
                    ValueCell::BrandedLink {
                        label: self.title.clone(),
                        url: self.url.clone(),
                        glyph: SimpleIcon::Wikipedia.into(),
                    },
                )]),
            ),
            Section::new(
                "Summary",
                dictionary_table(
                    self.summary
                        .as_deref()
                        .map(|summary| LabeledValue::new(" ", summary)),
                ),
            ),
        ]
    }
}

impl From<&WikipediaArticle> for Article {
    fn from(article: &WikipediaArticle) -> Self {
        Self {
            title: article.title.clone(),
            url: article.url.clone(),
            summary: article.summary.clone(),
        }
    }
}
