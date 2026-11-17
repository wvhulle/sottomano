use corredo::popovers::{NotchedPanelRegions, SatelliteRole};
use iced::{Element, border::Radius};

use crate::desktop_panels::{
    region_reporter::{BackdropBlur, PointerInput, RegionRoles},
    region_translation::translated_regions,
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct SatelliteBlur {
    pub item_list: BackdropBlur,
    pub beside: BackdropBlur,
}

pub(crate) fn satellite_with_regions<'a, Message: 'a>(
    satellite: Element<'a, Message>,
    satellite_blur: SatelliteBlur,
) -> Element<'a, Message> {
    translated_regions(
        satellite,
        move |regions: &NotchedPanelRegions, operation| {
            let backdrop_blur = match regions.role {
                SatelliteRole::ItemList => satellite_blur.item_list,
                SatelliteRole::Beside => satellite_blur.beside,
            };
            RegionRoles {
                pointer_input: PointerInput::PassedThrough,
                backdrop_blur,
                corner_radius: regions.surface_corners,
            }
            .report(&regions.surface_rows, regions.bounds, operation);
            RegionRoles {
                pointer_input: PointerInput::Accepted,
                backdrop_blur: BackdropBlur::Clear,
                corner_radius: Radius::default(),
            }
            .report(&[regions.accepting_input], regions.bounds, operation);
        },
    )
}
