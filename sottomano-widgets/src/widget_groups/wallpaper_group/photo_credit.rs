use chrono::Locale;
use sottomano_desktop_sources::wallpaper_place_article::Wallpaper;

use crate::{
    clock::Clock,
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        simple_icons::SimpleIcon,
        value_cells::{LabeledValue, ValueCell},
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct Photo {
    pub(super) file_name: String,
    photographer: Option<String>,
    caption: Option<String>,
    url: String,
}

impl Photo {
    pub(super) fn reading_row(&self, _clock: Clock, _locale: Locale) -> ReadingRow<'_> {
        ReadingRow {
            readout: RowReadout {
                label: match &self.photographer {
                    Some(text) if !text.is_empty() => ReadoutLabel::Labeled(text.clone()),
                    Some(_) | None => ReadoutLabel::Unlabeled,
                },
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                "Wallpaper photo",
                SimpleIcon::Unsplash.into(),
                ReadingDisplayState::Idle,
                Box::new(move |_| self.hover_card()),
            )
        }
    }

    pub(super) fn hover_card(&self) -> Vec<Section> {
        vec![
            Section::new(
                "Photo",
                dictionary_table(
                    [
                        Some(LabeledValue::new("File", self.file_name.as_str())),
                        self.photographer
                            .as_deref()
                            .map(|photographer| LabeledValue::new("Photographer", photographer)),
                        Some(LabeledValue::new(
                            "Unsplash",
                            ValueCell::BrandedLink {
                                label: "photo page".to_owned(),
                                url: self.url.clone(),
                                glyph: SimpleIcon::Unsplash.into(),
                            },
                        )),
                    ]
                    .into_iter()
                    .flatten(),
                ),
            ),
            Section::new(
                "Caption",
                dictionary_table(
                    self.caption
                        .as_deref()
                        .map(|caption| LabeledValue::new(" ", caption)),
                ),
            ),
        ]
    }
}

impl From<&Wallpaper> for Photo {
    fn from(wallpaper: &Wallpaper) -> Self {
        Self {
            file_name: wallpaper.file_name.clone(),
            photographer: wallpaper.photographer.clone(),
            caption: wallpaper.caption.clone(),
            url: wallpaper.photo_url.clone(),
        }
    }
}
