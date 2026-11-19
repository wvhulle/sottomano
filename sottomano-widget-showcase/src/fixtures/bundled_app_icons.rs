use std::path::Path;

use sottomano_desktop_sources::theme_icons::icon_requests::{APP_ICON_SIDE_PX, app_icon_request};
use sottomano_widgets::icon_catalog::{
    drawable_icon_catalog::{DrawableIcon, IconFreshness, SeededIcon},
    icon_handle::IconHandle,
};

const SHOWCASE_APP_ICONS: &str = env!("SHOWCASE_APP_ICONS");

pub fn bundled_app_icon(app_id: &str) -> SeededIcon {
    SeededIcon {
        request: app_icon_request(app_id),
        drawable_icon: DrawableIcon {
            handle: IconHandle::at(&Path::new(SHOWCASE_APP_ICONS).join(format!("{app_id}.svg"))),
            design_side_px: f32::from(APP_ICON_SIDE_PX),
            freshness: IconFreshness::Current,
        },
    }
}
