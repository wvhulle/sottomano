//! Niri window list.

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use iced::{
    Alignment, Element,
    font::Style as FontStyle,
    widget::{Row, image, mouse_area, rich_text, svg},
};
use niri_ipc::state::EventStreamState;

use crate::{
    bar_program::Message,
    data_sources::{
        app_rules::{self, AppRule, IconCandidate, TitleParts},
        favicons::{FaviconKind, RawFavicon},
        niri,
    },
    layer_surface::Position,
    user_config::{Config, font},
    widgets::{activity_line::activity_line, pager_slot::PagerSlot, text::fallback_spans},
};

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
            FaviconKind::Raster => FaviconHandle::Raster(image::Handle::from_bytes(raw.bytes.clone())),
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

    fn view<'a>(&self, side: f32) -> Element<'a, Message> {
        match &self.handle {
            FaviconHandle::Raster(h) => image(h.clone()).width(side).height(side).into(),
            FaviconHandle::Svg(h) => svg(h.clone()).width(side).height(side).into(),
        }
    }
}

/// The icon maps a window button draws from, bundled so they thread through the
/// per-button helpers as one cheap `Copy` handle instead of three arguments.
#[derive(Clone, Copy)]
struct Icons<'a> {
    app: &'a HashMap<String, PathBuf>,
    theme: &'a HashMap<String, PathBuf>,
    favicons: &'a HashMap<String, Favicon>,
}

pub fn view<'a>(
    niri: &EventStreamState,
    output: &str,
    config: &Config,
    app_icons: &'a HashMap<String, PathBuf>,
    theme_icons: &'a HashMap<String, PathBuf>,
    favicons: &'a HashMap<String, Favicon>,
    position: Position,
) -> Option<Element<'a, Message>> {
    let workspace = niri::active_workspace_id(niri, output)?;
    let windows = niri::windows_on_workspace(niri, workspace);
    if windows.is_empty() {
        return None;
    }
    let icons = Icons {
        app: app_icons,
        theme: theme_icons,
        favicons,
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

/// One window's clickable button: its content (icon + title) laid over the
/// focus underline, as a single element that focuses the window when pressed.
fn window_button<'a>(
    config: &Config,
    icons: Icons<'a>,
    win: &niri::WinEntry,
    position: Position,
) -> Element<'a, Message> {
    let rule = app_rules::rule_for(&win.app_id);
    let content = button_content(config, icons, rule, win);
    let focus = focus_line(config, win);
    mouse_area(PagerSlot::new(content, focus).view(position))
        .on_press(Message::FocusWindow(win.id))
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
) -> Element<'a, Message> {
    let icon = window_icon(config, icons, rule, win);
    let title = title_element(config, &rule.rewrite(label_for(win)));
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
    icon: Option<Element<'a, Message>>,
    title: Element<'a, Message>,
) -> Element<'a, Message> {
    Row::new()
        .extend(icon)
        .push(title)
        .spacing(config.layout.horizontal_widget_spacing_px * 0.6)
        .align_y(Alignment::Center)
        .into()
}

/// The focus/urgency underline for a window: fully shown when urgent or
/// focused, invisible otherwise.
fn focus_line<'a>(config: &Config, win: &niri::WinEntry) -> Element<'a, Message> {
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
fn title_element<'a>(config: &Config, parts: &TitleParts) -> Element<'a, Message> {
    let dim = config.palette.dim();
    let lead = (!parts.lead.is_empty()).then(|| {
        let italic = iced::Font {
            style: FontStyle::Italic,
            ..font::iced_font()
        };
        fallback_spans(&parts.lead, italic, Some(dim))
            .into_iter()
            .chain(fallback_spans(" · ", font::iced_font(), Some(dim)))
    });
    let spans: Vec<_> = lead
        .into_iter()
        .flatten()
        .chain(fallback_spans(&parts.main, font::iced_font(), None))
        .collect();
    rich_text(spans).size(font::size()).into()
}

fn window_icon<'a>(
    config: &Config,
    icons: Icons<'a>,
    rule: AppRule,
    win: &niri::WinEntry,
) -> Option<Element<'a, Message>> {
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
