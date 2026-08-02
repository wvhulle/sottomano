use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use iced::{
    Alignment, Element, Font, Subscription, Task,
    font::Style as FontStyle,
    task::Handle,
    widget::{Row, image, mouse_area, rich_text, svg},
};
use niri_ipc::state::EventStreamState;

use crate::{
    config::resolved::{ResolvedConfig, ResolvedFontConfig, fallback_font},
    data::source::{
        app_rules::{self, AppRule, IconCandidate, TitleParts},
        desktop_entries::DesktopIndex,
        favicons::{self, FaviconKind, RawFavicon},
        niri,
    },
    message_subscriptions::blocking,
    wayland_layers::surface::Position,
    widgets::{
        activity_line::activity_line,
        pager_slot::PagerSlot,
        text::fallback_spans,
        window_list_viewport::{Metrics, window_list_viewport},
    },
};

const ICONS_BACKSTOP: Duration = Duration::from_secs(20);

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

    pub fn is(&self, raw: &RawFavicon) -> bool {
        *self.bytes == *raw.bytes
    }

    fn view<'a>(&self, side: f32, alpha: f32) -> Element<'a, WindowListMessage> {
        match &self.handle {
            FaviconHandle::Raster(h) => image(h.clone())
                .width(side)
                .height(side)
                .opacity(alpha)
                .into(),
            FaviconHandle::Svg(h) => svg(h.clone())
                .width(side)
                .height(side)
                .opacity(alpha)
                .into(),
        }
    }
}

#[derive(Default)]
struct Resolves {
    favicons: Option<Handle>,
    fallback_fonts: Option<Handle>,
}

#[derive(Default)]
pub struct WindowList {
    favicons: HashMap<String, Favicon>,
    fallback_fonts: HashMap<char, Font>,
    scroll: f32,
    metrics: Metrics,
    resolves: Resolves,
}

#[derive(Debug, Clone)]
pub enum WindowListMessage {
    Clicked(u64),
    FaviconsLoaded(HashMap<String, RawFavicon>),
    FallbackFontsResolved(HashMap<char, Font>),
    IconsBackstopElapsed,
    Scrolled(f32),
    MetricsChanged(Metrics),
}

pub enum Action {
    None,
    FocusWindow(u64),
    RefreshIcons,
}

impl WindowList {
    pub fn subscription() -> Subscription<WindowListMessage> {
        iced::time::every(ICONS_BACKSTOP).map(|_| WindowListMessage::IconsBackstopElapsed)
    }

    pub fn update(&mut self, msg: WindowListMessage) -> Action {
        match msg {
            WindowListMessage::Clicked(window) => Action::FocusWindow(window),
            WindowListMessage::FaviconsLoaded(raw) => {
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
                Action::None
            }
            WindowListMessage::FallbackFontsResolved(map) => {
                self.fallback_fonts.extend(map);
                Action::None
            }
            WindowListMessage::IconsBackstopElapsed => Action::RefreshIcons,
            WindowListMessage::Scrolled(offset) => {
                self.scroll = offset.clamp(0.0, self.metrics.max_scroll());
                Action::None
            }
            WindowListMessage::MetricsChanged(metrics) => {
                self.metrics = metrics;
                self.scroll = self.scroll.clamp(0.0, self.metrics.max_scroll());
                Action::None
            }
        }
    }

    pub fn refresh(
        &mut self,
        niri: &EventStreamState,
        font: &ResolvedFontConfig,
    ) -> Task<WindowListMessage> {
        Task::batch([
            self.poll_favicons(niri),
            self.resolve_fallback_fonts(niri, font),
        ])
    }

    fn poll_favicons(&mut self, niri: &EventStreamState) -> Task<WindowListMessage> {
        let keys = favicons::firefox_keys(niri);
        let (task, handle) = blocking("favicons", move || favicons::resolve(keys))
            .map(WindowListMessage::FaviconsLoaded)
            .abortable();
        self.resolves.favicons = Some(handle.abort_on_drop());
        task
    }

    fn resolve_fallback_fonts(
        &mut self,
        niri: &EventStreamState,
        font: &ResolvedFontConfig,
    ) -> Task<WindowListMessage> {
        let pending: Vec<char> = niri::all_windows(niri)
            .iter()
            .flat_map(|win| win.title.chars())
            .filter(|ch| !font.covered.contains(ch) && !self.fallback_fonts.contains_key(ch))
            .collect();
        if pending.is_empty() {
            return Task::none();
        }
        let base = font.iced_font;
        let (task, handle) = blocking("fallback fonts", move || {
            pending
                .into_iter()
                .map(|ch| (ch, fallback_font(ch).unwrap_or(base)))
                .collect::<HashMap<char, Font>>()
        })
        .map(WindowListMessage::FallbackFontsResolved)
        .abortable();
        self.resolves.fallback_fonts = Some(handle.abort_on_drop());
        task
    }

    pub fn view<'a>(
        &'a self,
        niri: &niri::Snapshot,
        index: &'a DesktopIndex,
        output: &str,
        config: &ResolvedConfig,
        position: Position,
    ) -> Option<Element<'a, WindowListMessage>> {
        let workspace = niri.active_workspace_id(output)?;
        let windows = niri.windows_on_workspace(workspace);
        if windows.is_empty() {
            return None;
        }
        let icons = Icons {
            index,
            favicons: &self.favicons,
            fallbacks: &self.fallback_fonts,
        };
        let focused_index = windows.iter().position(|win| win.is_focused);
        let buttons = windows.iter().enumerate().map(|(i, win)| {
            let alpha = self.metrics.item_alpha(self.scroll, i);
            window_button(config, icons, win, position, alpha)
        });
        let row = Row::with_children(buttons)
            .spacing(config.layout.horizontal_widget_spacing_px * 2.0)
            .align_y(Alignment::Center);
        Some(window_list_viewport(
            row,
            self.scroll,
            focused_index,
            WindowListMessage::Scrolled,
            WindowListMessage::MetricsChanged,
        ))
    }
}

#[derive(Clone, Copy)]
struct Icons<'a> {
    index: &'a DesktopIndex,
    favicons: &'a HashMap<String, Favicon>,
    fallbacks: &'a HashMap<char, Font>,
}

fn window_button<'a>(
    config: &ResolvedConfig,
    icons: Icons<'a>,
    win: &niri::WinEntry,
    position: Position,
    alpha: f32,
) -> Element<'a, WindowListMessage> {
    let rule = app_rules::rule_for(&win.app_id);
    let content = button_content(config, icons, rule, win, alpha);
    let focus = focus_line(config, win, alpha);
    mouse_area(PagerSlot::new(content, focus).view(position))
        .on_press(WindowListMessage::Clicked(win.id))
        .into()
}

fn button_content<'a>(
    config: &ResolvedConfig,
    icons: Icons<'a>,
    rule: AppRule,
    win: &niri::WinEntry,
    alpha: f32,
) -> Element<'a, WindowListMessage> {
    let icon = window_icon(config, icons, rule, win, alpha);
    let parts = rule.rewrite(label_for(win));
    let title = match rule {
        AppRule::Terminal => title_element(config, &parts, icons.fallbacks, false, alpha),
        AppRule::Firefox | AppRule::Browser { .. } | AppRule::Fallback => {
            title_element(config, &parts, icons.fallbacks, true, alpha)
        }
    };
    icon_beside_title(config, icon, title)
}

fn icon_beside_title<'a>(
    config: &ResolvedConfig,
    icon: Option<Element<'a, WindowListMessage>>,
    title: Element<'a, WindowListMessage>,
) -> Element<'a, WindowListMessage> {
    Row::new()
        .extend(icon)
        .push(title)
        .spacing(config.layout.horizontal_widget_spacing_px * 1.5)
        .align_y(Alignment::Center)
        .into()
}

fn focus_line<'a>(
    config: &ResolvedConfig,
    win: &niri::WinEntry,
    alpha: f32,
) -> Element<'a, WindowListMessage> {
    let (color, opacity) = if win.is_urgent {
        (config.palette.urgent, 1.0)
    } else if win.is_focused {
        (config.palette.focus, 1.0)
    } else {
        (config.palette.focus, 0.0)
    };
    activity_line(
        color,
        opacity * alpha,
        config.layout.focus_underline_thickness_px,
    )
}

fn label_for(win: &niri::WinEntry) -> &str {
    if win.title.is_empty() {
        &win.app_id
    } else {
        &win.title
    }
}

fn title_element<'a>(
    config: &ResolvedConfig,
    parts: &TitleParts,
    fallbacks: &HashMap<char, Font>,
    lead_first: bool,
    alpha: f32,
) -> Element<'a, WindowListMessage> {
    let context_color = config.palette.inactive.scale_alpha(alpha);
    let main_color = config.palette.bar_foreground.scale_alpha(alpha);
    let font = &config.font;
    let main = fallback_spans(
        font,
        &parts.main,
        font.iced_font,
        Some(main_color),
        fallbacks,
    );
    let spans: Vec<_> = if parts.lead.is_empty() {
        main.into_iter().collect()
    } else {
        let italic = iced::Font {
            style: FontStyle::Italic,
            ..font.iced_font
        };
        let context = fallback_spans(font, &parts.lead, italic, Some(context_color), fallbacks);
        let separator = fallback_spans(font, " · ", font.iced_font, Some(context_color), fallbacks);
        if lead_first {
            context.into_iter().chain(separator).chain(main).collect()
        } else {
            main.into_iter().chain(separator).chain(context).collect()
        }
    };
    rich_text(spans).size(font.text_size_px).into()
}

fn window_icon<'a>(
    config: &ResolvedConfig,
    icons: Icons<'a>,
    rule: AppRule,
    win: &niri::WinEntry,
    alpha: f32,
) -> Option<Element<'a, WindowListMessage>> {
    let side = config.font.icon_size_px;
    if let Some(favicon) = rule
        .favicon_key(&win.title)
        .and_then(|key| icons.favicons.get(&key))
    {
        return Some(favicon.view(side, alpha));
    }
    let path = rule
        .icon_candidates(win)
        .into_iter()
        .find_map(|c| resolve_candidate(icons, c))?;
    let is_svg = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"));
    Some(if is_svg {
        svg(svg::Handle::from_path(path))
            .width(side)
            .height(side)
            .opacity(alpha)
            .into()
    } else {
        image(image::Handle::from_path(path))
            .width(side)
            .height(side)
            .opacity(alpha)
            .into()
    })
}

fn resolve_candidate(icons: Icons, candidate: IconCandidate) -> Option<PathBuf> {
    match candidate {
        IconCandidate::Theme(name) => icons.index.theme_icon_path(&name).map(Path::to_path_buf),
        IconCandidate::AppId(app_id) => icons.index.app_icon(&app_id).map(Path::to_path_buf),
    }
}
