use std::path::{Path, PathBuf};

use iced::Element;
use iced_controller_diagram::ControllerArtwork;
use sottomano_desktop_sources::desktop_windows_apps::{
    niri_window_snapshot, window_title_labels::IconCandidate,
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

impl WindowAssets<'_> {
    pub fn path_for(&self, win: &niri_window_snapshot::WinEntry) -> Option<PathBuf> {
        win.icon_candidates()
            .into_iter()
            .find_map(|candidate| match candidate {
                IconCandidate::Theme(name) => self.entries.theme_icon_path(&name),
                IconCandidate::AppId(app_id) => self.entries.app_icon(&app_id),
            })
            .map(Path::to_path_buf)
    }
}

#[must_use]
pub fn window_icon<'a, Message: 'a>(
    favicons: &Favicons,
    favicon_key: Option<&str>,
    icon: Option<&Path>,
    side: f32,
    opacity: f32,
) -> Option<Element<'a, Message>> {
    if let Some(favicon) = favicon_key.and_then(|key| favicons.get(key)) {
        return Some(favicon.view(side, opacity));
    }
    Some(IconHandle::at(icon?).view(side, opacity))
}
