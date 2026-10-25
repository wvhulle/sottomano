use chrono::Locale;
use sottomano_sources::wallpaper_place_article::Wallpaper;

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        simple_icons::SimpleIcon,
        tooltip_body::{Cell, KeyValue, Section, TooltipBody},
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct Article {
    title: String,
    url: String,
    summary: Option<String>,
}

impl BarWidget for Article {
    fn name(&self) -> &'static str {
        "Wallpaper article"
    }

    fn icon(&self) -> Glyph {
        SimpleIcon::Wikipedia.into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(self.title.clone())
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([
            Section::dictionary([KeyValue::new(
                "Article",
                Cell::Link {
                    label: self.title.clone(),
                    url: self.url.clone(),
                    glyph: Some(SimpleIcon::Wikipedia.into()),
                },
            )])
            .titled("Wikipedia"),
            Section::dictionary(
                self.summary
                    .as_deref()
                    .map(|summary| KeyValue::new(" ", summary)),
            )
            .titled("Summary"),
        ])
    }
}

impl Article {
    pub(super) fn of(wallpaper: &Wallpaper) -> Option<Self> {
        let article = wallpaper.article.as_ref()?;
        Some(Self {
            title: article.title.clone(),
            url: article.url.clone(),
            summary: article.summary.clone(),
        })
    }
}
