//! odin's `QuickLaunch`: a per-workspace row of apps launched often here but
//! not open right now. The bar handle is a rocket; toggling it unfolds the
//! ranked app icons to its right, each dimmed until hovered and launched on
//! click.
//!
//! Ranking lives in [`LaunchTracker`](crate::data::source::launch_tracker): the
//! app ranks each output's candidates in memory and pushes the results here via
//! [`set_ranked`](QuickLaunch::set_ranked); icon paths come from the app-wide
//! `DesktopIndex` cache at view time. Ranked lists are keyed by output name
//! (each output's active workspace differs).

use std::{collections::HashMap, path::Path, sync::Arc};

use iced::{
    Alignment, Element, Task,
    widget::{Row, image, mouse_area, svg},
};

use crate::{
    config::resolved::ResolvedConfig,
    data::source::{app_icon::DesktopEntries, desktop_entries::DesktopIndex, niri},
    message_subscriptions::blocking,
    wayland_layers::surface::Position,
    widgets::{
        icon::Icon,
        menu_group::{MenuGroup, MenuMessage},
    },
};

/// How many ranked icons to show at most (odin's `maxItems`).
pub const MAX_ITEMS: usize = 4;
/// Resting opacity of a ranked icon, brightened to full on hover, so closed
/// apps read as secondary to the live window list beside them (odin's `0.65`).
const RESTING_OPACITY: f32 = 0.65;

pub struct QuickLaunch {
    /// Ranked app-ids per output, pushed in by the app.
    ranked: HashMap<String, Vec<String>>,
    /// The shared `.desktop` index, kept for launching (`Exec` lookup) — icon
    /// paths come from the app-wide cache instead.
    desktop_entries: Option<Arc<DesktopEntries>>,
    hovered: Option<String>,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone)]
pub enum QuickLaunchMessage {
    Menu(MenuMessage),
    IconHover(Option<String>),
    /// A ranked icon was clicked; the group resolves it to a launch itself (it
    /// owns the desktop index and the niri-spawn fallback).
    Launch(String),
}

impl QuickLaunch {
    pub fn new(menu_expand_ms: u64) -> Self {
        Self {
            ranked: HashMap::new(),
            desktop_entries: None,
            hovered: None,
            menu: MenuGroup::new(menu_expand_ms),
        }
    }

    pub fn set_desktop_entries(&mut self, entries: Arc<DesktopEntries>) {
        self.desktop_entries = Some(entries);
    }

    pub fn set_ranked(&mut self, ranked: HashMap<String, Vec<String>>) {
        self.ranked = ranked;
    }

    pub fn icon_keys(&self) -> Vec<String> {
        self.ranked.values().flatten().cloned().collect()
    }

    pub fn update(&mut self, msg: QuickLaunchMessage) -> Task<QuickLaunchMessage> {
        match msg {
            QuickLaunchMessage::Menu(m) => {
                self.menu.update(m);
            }
            QuickLaunchMessage::IconHover(app_id) => self.hovered = app_id,
            QuickLaunchMessage::Launch(app_id) => return self.launch(app_id),
        }
        Task::none()
    }

    /// Launch a ranked app through niri: resolve its command from the desktop
    /// entry's `Exec` (falling back to the bare app-id), then hand it to
    /// `niri msg action spawn`. Spawning via the compositor lands the window in
    /// niri's scope and avoids forking a new process from vetrata's own
    /// GPU-heavy address space (slow, and it would make the app a child of the
    /// bar). Fire-and-forget — the launched window arrives back as a niri
    /// event.
    fn launch(&self, app_id: String) -> Task<QuickLaunchMessage> {
        let command = self
            .desktop_entries
            .as_ref()
            .and_then(|entries| entries.exec_command(&app_id))
            .unwrap_or_else(|| vec![app_id]);
        blocking("quicklaunch launch", move || niri::spawn(command)).discard()
    }

    pub fn view<'a>(
        &'a self,
        index: &'a DesktopIndex,
        output: &str,
        config: &ResolvedConfig,
        position: Position,
    ) -> Option<Element<'a, QuickLaunchMessage>> {
        let app_ids = self.ranked.get(output).filter(|ids| !ids.is_empty())?;

        let handle = mouse_area(
            Icon::Rocket
                .solid()
                .size(config.font.icon_size_px)
                .color(config.palette.bar_foreground),
        )
        .on_press(QuickLaunchMessage::Menu(MenuMessage::Toggle))
        .into();

        let reveal = self.menu.reveal();
        let content = Row::with_children(
            app_ids
                .iter()
                .filter_map(|app_id| self.icon_slot(config, index, app_id, reveal)),
        )
        .spacing(config.layout.horizontal_widget_spacing_px)
        .align_y(Alignment::Center);

        // Hover on the whole group only brightens the activity line (via the
        // menu); the toggle lives on the handle so clicking an icon launches it
        // without also toggling the menu.
        Some(self.menu.view(
            config,
            position,
            reveal,
            handle,
            content.into(),
            QuickLaunchMessage::Menu,
        ))
    }

    /// One ranked icon: dimmed until hovered, fading in with the reveal, that
    /// launches its app on click. `None` until its icon has resolved.
    fn icon_slot<'a>(
        &self,
        config: &ResolvedConfig,
        index: &DesktopIndex,
        app_id: &str,
        reveal: f32,
    ) -> Option<Element<'a, QuickLaunchMessage>> {
        let path = index.app_icon(app_id)?;
        let side = config.font.icon_size_px;
        let hovered = self.hovered.as_deref() == Some(app_id);
        let opacity = reveal * if hovered { 1.0 } else { RESTING_OPACITY };
        let icon = icon_image(path, side, opacity);
        let app_id = app_id.to_string();
        Some(
            mouse_area(icon)
                .on_press(QuickLaunchMessage::Launch(app_id.clone()))
                .on_enter(QuickLaunchMessage::IconHover(Some(app_id)))
                .on_exit(QuickLaunchMessage::IconHover(None))
                .into(),
        )
    }
}

/// An icon file as a sized, faded widget. Themed icons ship scalable SVGs while
/// hicolor app icons are raster; iced decodes them through different widgets,
/// so the extension picks the path (as in the window list).
fn icon_image<'a>(path: &Path, side: f32, opacity: f32) -> Element<'a, QuickLaunchMessage> {
    let is_svg = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"));
    if is_svg {
        svg(svg::Handle::from_path(path))
            .width(side)
            .height(side)
            .opacity(opacity)
            .into()
    } else {
        image(image::Handle::from_path(path))
            .width(side)
            .height(side)
            .opacity(opacity)
            .into()
    }
}
