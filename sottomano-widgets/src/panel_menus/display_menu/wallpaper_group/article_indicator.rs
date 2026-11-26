use sottomano_desktop_sources::wallpaper_place_article::unsplash_photo_article::{
    ArticleSummary, PlaceArticle, WikipediaArticle,
};

use super::WallpaperGroup;
use crate::{
    glyph_icons::simple_icons::SimpleIcon,
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::value_cells::{LabeledValue, ValueCell},
};

#[derive(Debug, Clone, PartialEq)]
pub struct Article {
    title: String,
    url: String,
    summary: ArticleSummary,
}

impl Article {
    pub(super) fn indicator(&self) -> GroupIndicator<'_> {
        GroupIndicator::glyph_only(
            "Wallpaper article",
            SimpleIcon::Wikipedia.into(),
            IndicatorState::Idle,
        )
        .with_readout(IndicatorReadout {
            label: match Some(&self.title) {
                Some(text) if !text.is_empty() => ReadoutLabel::Labeled(text.clone()),
                Some(_) | None => ReadoutLabel::Unlabeled,
            },
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
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
                dictionary_table(match &self.summary {
                    ArticleSummary::Extract(summary) => {
                        Some(LabeledValue::new(" ", summary.as_str()))
                    }
                    ArticleSummary::Unextracted => None,
                }),
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

#[derive(Debug, Clone, PartialEq)]
pub enum WallpaperArticle {
    Found(Article),
    Unmatched,
}

impl From<&PlaceArticle> for WallpaperArticle {
    fn from(place_article: &PlaceArticle) -> Self {
        match place_article {
            PlaceArticle::Found(article) => WallpaperArticle::Found(Article::from(article)),
            PlaceArticle::Unmatched => WallpaperArticle::Unmatched,
        }
    }
}

impl WallpaperGroup {
    pub(super) fn article_rows(&self) -> Vec<GroupIndicator<'_>> {
        let Some(lookup) = self.lookup.get() else {
            return Vec::new();
        };
        let WallpaperArticle::Found(article) = &lookup.article else {
            return Vec::new();
        };
        vec![article.indicator().with_hover_card(Box::new(move |_| {
            self.lookup.hover_card_with(article.hover_card())
        }))]
    }
}
