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
pub struct Photo {
    pub(super) file_name: String,
    photographer: Option<String>,
    caption: Option<String>,
    url: String,
}

impl BarWidget for Photo {
    fn name(&self) -> &'static str {
        "Wallpaper photo"
    }

    fn icon(&self) -> Glyph {
        SimpleIcon::Unsplash.into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        self.photographer.clone()
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([
            Section::dictionary(
                [
                    Some(KeyValue::new("File", self.file_name.as_str())),
                    self.photographer
                        .as_deref()
                        .map(|photographer| KeyValue::new("Photographer", photographer)),
                    Some(KeyValue::new(
                        "Unsplash",
                        Cell::Link {
                            label: "photo page".to_owned(),
                            url: self.url.clone(),
                            glyph: Some(SimpleIcon::Unsplash.into()),
                        },
                    )),
                ]
                .into_iter()
                .flatten(),
            )
            .titled("Photo"),
            Section::dictionary(
                self.caption
                    .as_deref()
                    .map(|caption| KeyValue::new(" ", caption)),
            )
            .titled("Caption"),
        ])
    }
}

impl Photo {
    pub(super) fn of(wallpaper: &Wallpaper) -> Self {
        Self {
            file_name: wallpaper.file_name.clone(),
            photographer: wallpaper.photographer.clone(),
            caption: wallpaper.caption.clone(),
            url: wallpaper.photo_url.clone(),
        }
    }
}
