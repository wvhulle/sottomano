use sottomano_desktop_sources::wallpaper_place_article::unsplash_photo_article::{
    PhotoPlaceName, PhotoPosition, Wallpaper,
};

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{localized_quantities::Quantity, value_cells::LabeledValue},
};

#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    name: String,
    position: PhotoPosition,
}

impl Place {
    pub(super) fn indicator(&self) -> GroupIndicator<'_> {
        GroupIndicator::glyph_only(
            "Wallpaper place",
            Phosphor::MapPin(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_readout(IndicatorReadout {
            label: match Some(&self.name) {
                Some(text) if !text.is_empty() => ReadoutLabel::Labeled(text.clone()),
                Some(_) | None => ReadoutLabel::Unlabeled,
            },
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub(super) fn hover_card(&self) -> Vec<Section> {
        let located_position = match self.position {
            PhotoPosition::Located(position) => Some(position),
            PhotoPosition::Unlocated => None,
        };
        let coordinates = located_position.into_iter().flat_map(|position| {
            [
                LabeledValue::new("Latitude", Quantity::angle(position.latitude).decimals(5)),
                LabeledValue::new("Longitude", Quantity::angle(position.longitude).decimals(5)),
            ]
        });
        vec![Section::new(
            "Location",
            dictionary_table(
                [LabeledValue::new("Where", self.name.as_str())]
                    .into_iter()
                    .chain(coordinates),
            ),
        )]
    }
}

impl Place {
    pub(super) fn wallpaper_place(wallpaper: &Wallpaper) -> Option<Self> {
        match &wallpaper.place {
            PhotoPlaceName::Named(name) => Some(Self {
                name: name.clone(),
                position: wallpaper.position,
            }),
            PhotoPlaceName::Unnamed => None,
        }
    }
}
