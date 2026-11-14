use std::borrow::Cow;

use chrono::Locale;
use sottomano_desktop_sources::wallpaper_place_article::Wallpaper;

use crate::{
    clock::Clock,
    group_items::group_item_reading::GroupItemReading,
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widgets::{
        icon_glyph::IconGlyph,
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

impl GroupItemReading for Article {
    fn name(&self) -> Cow<'static, str> {
        "Wallpaper article".into()
    }

    fn icon(&self) -> IconGlyph {
        SimpleIcon::Wikipedia.into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(self.title.clone())
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([
            Section::dictionary([LabeledValue::new(
                "Article",
                ValueCell::BrandedLink {
                    label: self.title.clone(),
                    url: self.url.clone(),
                    glyph: SimpleIcon::Wikipedia.into(),
                },
            )])
            .titled("Wikipedia"),
            Section::dictionary(
                self.summary
                    .as_deref()
                    .map(|summary| LabeledValue::new(" ", summary)),
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
