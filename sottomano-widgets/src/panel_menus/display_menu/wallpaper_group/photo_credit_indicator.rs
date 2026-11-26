use sottomano_desktop_sources::wallpaper_place_article::unsplash_photo_article::{
    PhotoCaption, PhotoCredit, Wallpaper,
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
pub struct Photo {
    pub(super) file_name: String,
    photographer: PhotoCredit,
    caption: PhotoCaption,
    url: String,
}

impl Photo {
    pub(super) fn indicator(&self) -> GroupIndicator<'_> {
        GroupIndicator::glyph_only(
            "Wallpaper photo",
            SimpleIcon::Unsplash.into(),
            IndicatorState::Idle,
        )
        .with_readout(IndicatorReadout {
            label: match &self.photographer {
                PhotoCredit::Photographer(text) if !text.is_empty() => {
                    ReadoutLabel::Labeled(text.clone())
                }
                PhotoCredit::Photographer(_) | PhotoCredit::Uncredited => ReadoutLabel::Unlabeled,
            },
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub(super) fn hover_card(&self) -> Vec<Section> {
        vec![
            Section::new(
                "Photo",
                dictionary_table(
                    [
                        Some(LabeledValue::new("File", self.file_name.as_str())),
                        match &self.photographer {
                            PhotoCredit::Photographer(photographer) => {
                                Some(LabeledValue::new("Photographer", photographer.as_str()))
                            }
                            PhotoCredit::Uncredited => None,
                        },
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
                dictionary_table(match &self.caption {
                    PhotoCaption::Described(caption) => {
                        Some(LabeledValue::new(" ", caption.as_str()))
                    }
                    PhotoCaption::Undescribed => None,
                }),
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

impl WallpaperGroup {
    pub(super) fn photo_credit_rows(&self) -> Vec<GroupIndicator<'_>> {
        let Some(lookup) = self.lookup.get() else {
            return Vec::new();
        };
        vec![lookup.photo.indicator().with_hover_card(Box::new(move |_| {
            self.lookup.hover_card_with(lookup.photo.hover_card())
        }))]
    }
}
