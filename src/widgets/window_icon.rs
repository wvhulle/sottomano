use std::path::{Path, PathBuf};

use iced::Element;

use crate::{
    data::source::{
        app_rules::{AppRule, IconCandidate},
        desktop_entries::DesktopIndex,
        niri,
    },
    widgets::{favicon::Favicons, icon_handle::IconHandle, text::FontFallbacks},
};

#[derive(Clone, Copy)]
pub struct WindowAssets<'a> {
    pub entries: &'a DesktopIndex,
    pub favicons: &'a Favicons,
    pub fallbacks: &'a FontFallbacks,
}

impl WindowAssets<'_> {
    pub fn path_for(&self, rule: AppRule, win: &niri::WinEntry) -> Option<PathBuf> {
        rule.icon_candidates(win)
            .into_iter()
            .find_map(|candidate| match candidate {
                IconCandidate::Theme(name) => self.entries.theme_icon_path(&name),
                IconCandidate::AppId(app_id) => self.entries.app_icon(&app_id),
            })
            .map(Path::to_path_buf)
    }
}

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
