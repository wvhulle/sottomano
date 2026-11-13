use iced::Element;
use iced_controller_diagram::ControllerArtwork;
use sottomano_desktop_sources::desktop_windows_apps::{
    window_title_labels::IconCandidate, workspace_window_snapshot,
};

use crate::{
    desktop_index::DesktopIndex,
    widgets::{favicon::Favicons, icon_handle::IconHandle},
};

#[derive(Clone, Copy)]
pub struct WindowAssets<'a> {
    pub entries: &'a DesktopIndex,
    pub favicons: &'a Favicons,
    pub controller_artwork: &'a ControllerArtwork,
}

impl<'a> WindowAssets<'a> {
    #[must_use]
    pub fn icon_for(
        &self,
        window_entry: &workspace_window_snapshot::WinEntry,
    ) -> Option<&'a IconHandle> {
        resolved_window_icon(self.entries, window_entry)
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
    if let Some(favicon) = favicon_key.and_then(|key| favicons.get(key)) {
        return Some(favicon.view(side, opacity));
    }
    Some(icon?.view(side, opacity))
}

#[must_use]
pub fn resolved_window_icon<'a>(
    icons: &'a DesktopIndex,
    window_entry: &workspace_window_snapshot::WinEntry,
) -> Option<&'a IconHandle> {
    window_entry
        .icon_candidates()
        .into_iter()
        .find_map(|candidate| match candidate {
            IconCandidate::Theme(name) => icons.theme_icon(&name),
            IconCandidate::AppId(app_id) => icons.app_icon(&app_id),
        })
}
