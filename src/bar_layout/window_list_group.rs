//! Niri window list: the clickable per-window buttons, plus the icon and
//! fallback-font pipelines that feed them.
//!
//! Resolution stays off the render path: every pipeline gathers its keys from
//! the current niri window set and resolves them in a [`blocking`] task, so
//! `view` only does map lookups. Each in-flight resolve carries an
//! abort-on-drop handle; replacing it cancels the superseded task, so a stale
//! result can never overwrite a newer map.

use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use iced::{
    Alignment, Element, Font, Subscription, Task,
    font::Style as FontStyle,
    task::Handle,
    widget::{Row, image, mouse_area, rich_text, svg},
};
use niri_ipc::state::EventStreamState;

use crate::{
    data_sources::{
        app_icon::{self, DesktopEntries},
        app_rules::{self, AppRule, IconCandidate, TitleParts},
        favicons::{self, FaviconKind, RawFavicon},
        niri,
    },
    layer_surface::Position,
    message_subscriptions::blocking,
    user_config::{Config, font},
    widgets::{activity_line::activity_line, pager_slot::PagerSlot, text::fallback_spans},
};

const ICONS_BACKSTOP: Duration = Duration::from_secs(20);
const ENTRIES_RELOAD: Duration = Duration::from_secs(300);

/// A resolved favicon ready to render: an iced handle built once from the blob,
/// plus the blob itself so a re-poll that finds the same bytes can keep this
/// same handle. Raster handles (`image::Handle::from_bytes`) get a fresh cache
/// id on every construction, so rebuilding one every poll would re-upload the
/// GPU texture; reusing the handle keeps the id — and thus the texture — stable.
#[derive(Clone)]
pub struct Favicon {
    bytes: Arc<[u8]>,
    handle: FaviconHandle,
}

#[derive(Clone)]
enum FaviconHandle {
    Raster(image::Handle),
    Svg(svg::Handle),
}

impl Favicon {
    /// Build the iced handle from the blob. Call once per distinct blob, not
    /// per frame (see the type-level note).
    pub fn new(raw: RawFavicon) -> Self {
        let handle = match raw.kind {
            FaviconKind::Raster => {
                FaviconHandle::Raster(image::Handle::from_bytes(raw.bytes.clone()))
            }
            FaviconKind::Svg => FaviconHandle::Svg(svg::Handle::from_memory(raw.bytes.clone())),
        };
        Self {
            bytes: Arc::from(raw.bytes),
            handle,
        }
    }

    /// Whether `raw` is byte-for-byte the favicon this was built from, so its
    /// handle can be reused instead of rebuilt.
    pub fn is(&self, raw: &RawFavicon) -> bool {
        *self.bytes == *raw.bytes
    }

    fn view<'a>(&self, side: f32) -> Element<'a, WindowListMessage> {
        match &self.handle {
            FaviconHandle::Raster(h) => image(h.clone()).width(side).height(side).into(),
            FaviconHandle::Svg(h) => svg(h.clone()).width(side).height(side).into(),
        }
    }
}

/// Abort-on-drop handles for the in-flight resolves.
#[derive(Default)]
struct Resolves {
    desktop_entries: Option<Handle>,
    favicons: Option<Handle>,
    app_icons: Option<Handle>,
    theme_icons: Option<Handle>,
    fallback_fonts: Option<Handle>,
}

#[derive(Default)]
pub struct WindowListGroup {
    desktop_entries: Option<Arc<DesktopEntries>>,
    favicons: HashMap<String, Favicon>,
    app_icons: HashMap<String, PathBuf>,
    theme_icons: HashMap<String, PathBuf>,
    /// Per-char fonts for glyphs the bar font lacks, resolved via fontconfig
    /// off-thread; chars fontconfig has no answer for map to the bar font so
    /// they are not re-queried.
    fallback_fonts: HashMap<char, Font>,
    resolves: Resolves,
}

#[derive(Debug, Clone)]
pub enum WindowListMessage {
    Clicked(u64),
    DesktopEntriesLoaded(Arc<DesktopEntries>),
    FaviconsLoaded(HashMap<String, RawFavicon>),
    AppIconsResolved(HashMap<String, PathBuf>),
    ThemeIconsResolved(HashMap<String, PathBuf>),
    FallbackFontsResolved(HashMap<char, Font>),
    IconsBackstopElapsed,
    EntriesReloadElapsed,
}

impl WindowListGroup {
    pub fn init(&mut self) -> Task<WindowListMessage> {
        self.load_desktop_entries()
    }

    /// Periodic backstop for the icon polls (in case a niri window-change
    /// event was missed) and the `.desktop`-index reload (so a newly installed
    /// app or a live theme switch is picked up without a restart).
    pub fn subscription() -> Subscription<WindowListMessage> {
        Subscription::batch([
            iced::time::every(ICONS_BACKSTOP).map(|_| WindowListMessage::IconsBackstopElapsed),
            iced::time::every(ENTRIES_RELOAD).map(|_| WindowListMessage::EntriesReloadElapsed),
        ])
    }

    pub fn update(
        &mut self,
        msg: WindowListMessage,
        niri: &EventStreamState,
    ) -> Task<WindowListMessage> {
        match msg {
            WindowListMessage::Clicked(_) => Task::none(),
            WindowListMessage::DesktopEntriesLoaded(entries) => {
                self.desktop_entries = Some(entries);
                self.app_icons.clear();
                self.theme_icons.clear();
                Task::batch([self.poll_app_icons(niri), self.poll_theme_icons(niri)])
            }
            WindowListMessage::FaviconsLoaded(raw) => {
                // Reuse the existing handle when a re-poll returns the same
                // blob, so its GPU texture stays cached (raster handles get a
                // fresh cache id on every rebuild — see `Favicon`).
                let previous = std::mem::take(&mut self.favicons);
                self.favicons = raw
                    .into_iter()
                    .map(|(title, raw)| {
                        let favicon = match previous.get(&title) {
                            Some(existing) if existing.is(&raw) => existing.clone(),
                            _ => Favicon::new(raw),
                        };
                        (title, favicon)
                    })
                    .collect();
                Task::none()
            }
            WindowListMessage::AppIconsResolved(map) => {
                self.app_icons = map;
                Task::none()
            }
            WindowListMessage::ThemeIconsResolved(map) => {
                self.theme_icons = map;
                Task::none()
            }
            WindowListMessage::FallbackFontsResolved(map) => {
                self.fallback_fonts.extend(map);
                Task::none()
            }
            WindowListMessage::IconsBackstopElapsed => self.refresh(niri),
            WindowListMessage::EntriesReloadElapsed => self.load_desktop_entries(),
        }
    }

    /// Re-run every pipeline against the current window set.
    pub fn refresh(&mut self, niri: &EventStreamState) -> Task<WindowListMessage> {
        Task::batch([
            self.poll_favicons(niri),
            self.poll_app_icons(niri),
            self.poll_theme_icons(niri),
            self.resolve_fallback_fonts(niri),
        ])
    }

    fn load_desktop_entries(&mut self) -> Task<WindowListMessage> {
        let (task, handle) = blocking("desktop entries", DesktopEntries::load)
            .map(|entries| WindowListMessage::DesktopEntriesLoaded(Arc::new(entries)))
            .abortable();
        self.resolves.desktop_entries = Some(handle.abort_on_drop());
        task
    }

    /// Firefox favicons, read synchronously off-thread from Firefox's
    /// history/favicon DBs, keyed by the brand-stripped page title.
    fn poll_favicons(&mut self, niri: &EventStreamState) -> Task<WindowListMessage> {
        let keys = favicons::firefox_keys(niri);
        let (task, handle) = blocking("favicons", move || favicons::resolve(keys))
            .map(WindowListMessage::FaviconsLoaded)
            .abortable();
        self.resolves.favicons = Some(handle.abort_on_drop());
        task
    }

    /// `.desktop`-entry app-id icons, against the last-loaded desktop-entry
    /// index. A no-op until that index has loaded.
    fn poll_app_icons(&mut self, niri: &EventStreamState) -> Task<WindowListMessage> {
        let Some(entries) = self.desktop_entries.clone() else {
            return Task::none();
        };
        let ids = app_icon::app_icon_keys(niri);
        let (task, handle) =
            blocking("app icons", move || app_icon::resolve_app_icons(&entries, ids))
                .map(WindowListMessage::AppIconsResolved)
                .abortable();
        self.resolves.app_icons = Some(handle.abort_on_drop());
        task
    }

    /// Icon-theme-name icons (e.g. a terminal's command-word icon). A no-op
    /// until the desktop-entry index has loaded.
    fn poll_theme_icons(&mut self, niri: &EventStreamState) -> Task<WindowListMessage> {
        let Some(entries) = self.desktop_entries.clone() else {
            return Task::none();
        };
        let names = app_icon::theme_icon_keys(niri);
        let (task, handle) = blocking("theme icons", move || {
            app_icon::resolve_theme_icons(&entries, names)
        })
        .map(WindowListMessage::ThemeIconsResolved)
        .abortable();
        self.resolves.theme_icons = Some(handle.abort_on_drop());
        task
    }

    /// Ask fontconfig (off-thread) which font to use for title glyphs the bar
    /// font lacks, so `view` renders them from a plain map lookup.
    fn resolve_fallback_fonts(&mut self, niri: &EventStreamState) -> Task<WindowListMessage> {
        let pending: Vec<char> = niri::all_windows(niri)
            .iter()
            .flat_map(|win| win.title.chars())
            .filter(|ch| !font::bar_font_covers(*ch) && !self.fallback_fonts.contains_key(ch))
            .collect();
        if pending.is_empty() {
            return Task::none();
        }
        let (task, handle) = blocking("fallback fonts", move || {
            pending
                .into_iter()
                .map(|ch| (ch, font::fallback_font(ch).unwrap_or_else(font::iced_font)))
                .collect::<HashMap<char, Font>>()
        })
        .map(WindowListMessage::FallbackFontsResolved)
        .abortable();
        self.resolves.fallback_fonts = Some(handle.abort_on_drop());
        task
    }

    pub fn view<'a>(
        &'a self,
        niri: &EventStreamState,
        output: &str,
        config: &Config,
        position: Position,
    ) -> Option<Element<'a, WindowListMessage>> {
        let workspace = niri::active_workspace_id(niri, output)?;
        let windows = niri::windows_on_workspace(niri, workspace);
        if windows.is_empty() {
            return None;
        }
        let icons = Icons {
            app: &self.app_icons,
            theme: &self.theme_icons,
            favicons: &self.favicons,
            fallbacks: &self.fallback_fonts,
        };
        let buttons = windows
            .iter()
            .map(|win| window_button(config, icons, win, position));
        Some(
            Row::with_children(buttons)
                .spacing(config.layout.horizontal_widget_spacing_px * 2.0)
                .align_y(Alignment::Center)
                .into(),
        )
    }
}

/// The lookup maps a window button draws from, bundled so they thread through
/// the per-button helpers as one cheap `Copy` handle.
#[derive(Clone, Copy)]
struct Icons<'a> {
    app: &'a HashMap<String, PathBuf>,
    theme: &'a HashMap<String, PathBuf>,
    favicons: &'a HashMap<String, Favicon>,
    fallbacks: &'a HashMap<char, Font>,
}

/// One window's clickable button: its content (icon + title) laid over the
/// focus underline, as a single element that focuses the window when pressed.
fn window_button<'a>(
    config: &Config,
    icons: Icons<'a>,
    win: &niri::WinEntry,
    position: Position,
) -> Element<'a, WindowListMessage> {
    let rule = app_rules::rule_for(&win.app_id);
    let content = button_content(config, icons, rule, win);
    let focus = focus_line(config, win);
    mouse_area(PagerSlot::new(content, focus).view(position))
        .on_press(WindowListMessage::Clicked(win.id))
        .into()
}

/// Assemble a window's parsed title and icon into the button's content. This
/// `match` is the per-app seam: each rule can arrange its pieces differently.
/// They currently share one icon-beside-title layout, so a new app that wants a
/// distinct arrangement gets its own arm here without disturbing the others.
fn button_content<'a>(
    config: &Config,
    icons: Icons<'a>,
    rule: AppRule,
    win: &niri::WinEntry,
) -> Element<'a, WindowListMessage> {
    let icon = window_icon(config, icons, rule, win);
    let title = title_element(config, &rule.rewrite(label_for(win)), icons.fallbacks);
    match rule {
        AppRule::Firefox
        | AppRule::Browser { .. }
        | AppRule::Terminal
        | AppRule::Fallback => icon_beside_title(config, icon, title),
    }
}

/// Icon (when resolved) and title in a horizontal row — the shared arrangement
/// every current app-rule uses.
fn icon_beside_title<'a>(
    config: &Config,
    icon: Option<Element<'a, WindowListMessage>>,
    title: Element<'a, WindowListMessage>,
) -> Element<'a, WindowListMessage> {
    Row::new()
        .extend(icon)
        .push(title)
        .spacing(config.layout.horizontal_widget_spacing_px * 0.6)
        .align_y(Alignment::Center)
        .into()
}

/// The focus/urgency underline for a window: fully shown when urgent or
/// focused, invisible otherwise.
fn focus_line<'a>(config: &Config, win: &niri::WinEntry) -> Element<'a, WindowListMessage> {
    let (color, opacity) = if win.is_urgent {
        (config.palette.urgent(), 1.0)
    } else if win.is_focused {
        (config.palette.focus(), 1.0)
    } else {
        (config.palette.focus(), 0.0)
    };
    activity_line(color, opacity, config.layout.focus_underline_thickness_px)
}

/// The window's own title, or its app-id when the title is empty — the string
/// each rule's `rewrite` parses.
fn label_for(win: &niri::WinEntry) -> &str {
    if win.title.is_empty() {
        &win.app_id
    } else {
        &win.title
    }
}

/// Lead (muted, italic) + "·" + main, mirroring odin's `WindowTitleContext` /
/// `WindowTitleBody`. The lead block is skipped entirely when there is no
/// rewritten prefix, so the main text sits alone with no stray dot.
fn title_element<'a>(
    config: &Config,
    parts: &TitleParts,
    fallbacks: &HashMap<char, Font>,
) -> Element<'a, WindowListMessage> {
    let dim = config.palette.dim();
    let lead = (!parts.lead.is_empty()).then(|| {
        let italic = iced::Font {
            style: FontStyle::Italic,
            ..font::iced_font()
        };
        fallback_spans(&parts.lead, italic, Some(dim), fallbacks)
            .into_iter()
            .chain(fallback_spans(" · ", font::iced_font(), Some(dim), fallbacks))
    });
    let spans: Vec<_> = lead
        .into_iter()
        .flatten()
        .chain(fallback_spans(&parts.main, font::iced_font(), None, fallbacks))
        .collect();
    rich_text(spans).size(font::size()).into()
}

fn window_icon<'a>(
    config: &Config,
    icons: Icons<'a>,
    rule: AppRule,
    win: &niri::WinEntry,
) -> Option<Element<'a, WindowListMessage>> {
    let side = config.font.icon_size_px;
    // A tab's favicon (Firefox) always wins over the app-id icon; it is rendered
    // straight from its in-memory blob rather than a resolved file path.
    if let Some(favicon) = rule
        .favicon_key(&win.title)
        .and_then(|key| icons.favicons.get(&key))
    {
        return Some(favicon.view(side));
    }
    let path = rule
        .icon_candidates(win)
        .into_iter()
        .find_map(|c| resolve_candidate(icons, c))?;
    // Icon themes (e.g. Colloid) ship scalable SVGs; hicolor app icons are
    // raster. iced's `image` widget only decodes raster formats, so SVGs must
    // go through the `svg` widget or they render blank.
    let is_svg = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg"));
    Some(if is_svg {
        svg(svg::Handle::from_path(path))
            .width(side)
            .height(side)
            .into()
    } else {
        image(image::Handle::from_path(path))
            .width(side)
            .height(side)
            .into()
    })
}

fn resolve_candidate(icons: Icons, candidate: IconCandidate) -> Option<PathBuf> {
    match candidate {
        IconCandidate::Theme(name) => icons.theme.get(&name).cloned(),
        IconCandidate::AppId(app_id) => icons.app.get(&app_id).cloned(),
    }
}
