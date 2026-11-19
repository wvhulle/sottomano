use sottomano_desktop_sources::wallpaper_place_article::Wallpaper;

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
    photographer: Option<String>,
    caption: Option<String>,
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
