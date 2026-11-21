use iced::Element;
use iced_controller_diagram::ControllerArtwork;
use sottomano_desktop_sources::{
    desktop_windows_apps::{
        desktop_entries::DesktopEntries, window_title_labels::IconCandidate,
        workspace_window_snapshot,
    },
    theme_icons::icon_requests::{app_icon_request, window_theme_icon_request},
};

use crate::icon_catalog::{
    drawable_icon_catalog::{DrawableIcon, IconCatalog},
    favicons::Favicons,
};

#[derive(Clone, Copy)]
pub struct WindowAssets<'assets> {
    pub icon_catalog: &'assets IconCatalog,
    pub desktop_entries: &'assets DesktopEntries,
    pub favicons: &'assets Favicons,
    pub controller_artwork: &'assets ControllerArtwork,
}

impl<'assets> WindowAssets<'assets> {
    #[must_use]
    pub fn icon_for(
        &self,
        window_entry: &workspace_window_snapshot::WinEntry,
    ) -> Option<&'assets DrawableIcon> {
        resolved_window_icon(self.icon_catalog, window_entry)
    }
}

#[must_use]
pub fn window_icon<'view, Message: 'view>(
    favicons: &Favicons,
    favicon_key: Option<&str>,
    icon: Option<&DrawableIcon>,
    side: f32,
    opacity: f32,
) -> Option<Element<'view, Message>> {
    if let Some(favicon) = favicon_key.and_then(|key| favicons.cached.get(key)) {
        return Some(favicon.view(side, opacity));
    }
    Some(icon?.view(side, opacity))
}

#[must_use]
pub fn resolved_window_icon<'assets>(
    icon_catalog: &'assets IconCatalog,
    window_entry: &workspace_window_snapshot::WinEntry,
) -> Option<&'assets DrawableIcon> {
    window_entry
        .icon_candidates()
        .into_iter()
        .find_map(|candidate| {
            let request = match candidate {
                IconCandidate::Theme(icon_name) => window_theme_icon_request(&icon_name),
                IconCandidate::AppId(app_id) => app_icon_request(&app_id),
            };
            icon_catalog.drawable_icon(&request)
        })
}
