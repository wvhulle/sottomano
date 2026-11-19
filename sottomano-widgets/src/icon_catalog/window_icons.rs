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
    drawable_icon_catalog::IconCatalog, favicons::Favicons, icon_handle::IconHandle,
};

#[derive(Clone, Copy)]
pub struct WindowAssets<'a> {
    pub icon_catalog: &'a IconCatalog,
    pub desktop_entries: &'a DesktopEntries,
    pub favicons: &'a Favicons,
    pub controller_artwork: &'a ControllerArtwork,
}

impl<'a> WindowAssets<'a> {
    #[must_use]
    pub fn icon_for(
        &self,
        window_entry: &workspace_window_snapshot::WinEntry,
    ) -> Option<&'a IconHandle> {
        resolved_window_icon(self.icon_catalog, window_entry)
    }
}

#[must_use]
pub fn window_icon<'a, Message: 'a>(
    favicons: &Favicons,
    favicon_key: Option<&str>,
    icon: Option<&IconHandle>,
    side: f32,
    opacity: f32,
) -> Option<Element<'a, Message>> {
    if let Some(favicon) = favicon_key.and_then(|key| favicons.cached.get(key)) {
        return Some(favicon.view(side, opacity));
    }
    Some(icon?.view(side, opacity))
}

#[must_use]
pub fn resolved_window_icon<'a>(
    icon_catalog: &'a IconCatalog,
    window_entry: &workspace_window_snapshot::WinEntry,
) -> Option<&'a IconHandle> {
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
        .map(|drawable_icon| &drawable_icon.handle)
}
