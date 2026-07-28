//! odin's `QuickLaunch`: a per-workspace row of apps launched often here but
//! not open right now. The bar handle is a rocket; toggling it unfolds the
//! ranked app icons to its right, each dimmed until hovered and launched on
//! click.
//!
//! Ranking lives in [`LaunchTracker`](crate::data_sources::launch_tracker): the
//! app ranks each output's candidates in memory and pushes the results here via
//! [`set_ranked`](QuickLaunch::set_ranked); this group only resolves their
//! icons (off-thread) and renders them. Ranked lists are keyed by output name
//! (each output's active workspace differs); icon resolution is global.

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use iced::{
    Alignment, Element, Task,
    task::Handle,
    widget::{Row, image, mouse_area, svg},
};

use crate::{
    config::resolved::ResolvedConfig,
    data_sources::{
        app_icon::{self, DesktopEntries},
        niri,
    },
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
    icons: HashMap<String, PathBuf>,
    desktop_entries: Option<Arc<DesktopEntries>>,
    hovered: Option<String>,
    pub(crate) menu: MenuGroup,
    /// Abort-on-drop handle for the in-flight icon resolve.
    resolve: Option<Handle>,
}

#[derive(Debug, Clone)]
pub enum QuickLaunchMessage {
    Menu(MenuMessage),
    IconHover(Option<String>),
    /// A ranked icon was clicked; the group resolves it to a launch itself (it
    /// owns the desktop index and the niri-spawn fallback).
    Launch(String),
    IconsResolved(HashMap<String, PathBuf>),
}

impl QuickLaunch {
    pub fn new(menu_expand_ms: u64) -> Self {
        Self {
            ranked: HashMap::new(),
            icons: HashMap::new(),
            desktop_entries: None,
            hovered: None,
            menu: MenuGroup::new(menu_expand_ms),
            resolve: None,
        }
    }

    /// Adopt the shared `.desktop` index and re-resolve icons for the current
    /// ranked set against it.
    pub fn set_desktop_entries(
        &mut self,
        entries: Arc<DesktopEntries>,
    ) -> Task<QuickLaunchMessage> {
        self.desktop_entries = Some(entries);
        self.resolve_icons()
    }

    /// Adopt freshly ranked candidates (per output) and resolve any icons they
    /// need that are not yet cached.
    pub fn set_ranked(&mut self, ranked: HashMap<String, Vec<String>>) -> Task<QuickLaunchMessage> {
        self.ranked = ranked;
        self.resolve_icons()
    }

    pub fn update(&mut self, msg: QuickLaunchMessage) -> Task<QuickLaunchMessage> {
        match msg {
            QuickLaunchMessage::Menu(m) => {
                self.menu.update(m);
            }
            QuickLaunchMessage::IconHover(app_id) => self.hovered = app_id,
            QuickLaunchMessage::IconsResolved(icons) => self.icons = icons,
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

    fn resolve_icons(&mut self) -> Task<QuickLaunchMessage> {
        let Some(entries) = self.desktop_entries.clone() else {
            return Task::none();
        };
        let mut ids: Vec<String> = self.ranked.values().flatten().cloned().collect();
        ids.sort_unstable();
        ids.dedup();
        if ids.is_empty() {
            return Task::none();
        }
        let (task, handle) = blocking("quicklaunch icons", move || {
            app_icon::resolve_app_icons(&entries, ids)
        })
        .map(QuickLaunchMessage::IconsResolved)
        .abortable();
        self.resolve = Some(handle.abort_on_drop());
        task
    }

    pub fn view<'a>(
        &'a self,
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
                .filter_map(|app_id| self.icon_slot(config, app_id, reveal)),
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
        app_id: &str,
        reveal: f32,
    ) -> Option<Element<'a, QuickLaunchMessage>> {
        let path = self.icons.get(app_id)?;
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
fn icon_image<'a>(path: &PathBuf, side: f32, opacity: f32) -> Element<'a, QuickLaunchMessage> {
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
