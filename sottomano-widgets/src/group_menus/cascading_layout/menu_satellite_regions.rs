use corredo::popovers::{NestedMenuSatellite, NotchedPanelRegions, SatelliteRole};
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
    satellite: NestedMenuSatellite<'a, Message>,
    satellite_blur: SatelliteBlur,
) -> Element<'a, Message> {
    let backdrop_blur = match satellite.role {
        SatelliteRole::ItemList => satellite_blur.item_list,
        SatelliteRole::Beside => satellite_blur.beside,
    };
    translated_regions(
        satellite.element,
        move |regions: &NotchedPanelRegions, operation| {
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
