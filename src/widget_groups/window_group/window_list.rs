use std::{
    collections::HashMap,
    mem::take,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use iced::{
    Alignment, Element, Font, Length, Subscription, Task,
    alignment::{Horizontal, Vertical},
    border::Radius,
    font::Style as FontStyle,
    task::Handle,
    time::every,
    widget::{Row, button, container, image, mouse_area, rich_text, stack, svg, text},
};
use niri_ipc::state::EventStreamState;

use crate::{
    config::{
        Config,
        font::{FontConfig, fallback_font},
    },
    data::source::{
        app_rules::{self, AppRule, IconCandidate, TitleParts},
        audio::WindowAudio,
        desktop_entries::DesktopIndex,
        favicons::{self, FaviconKind, RawFavicon},
        font_awesome::Solid,
        job_view::{self, Jobs},
        launcher_entry::{self, LauncherEntries, LauncherEntry},
        niri,
    },
    message_subscriptions::blocking,
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage, hoverable},
    },
    widgets::{
        activity_line::activity_line,
        flat_button,
        focus_indicator::focus_indicator,
        pager_slot::PagerSlot,
        progress_line::progress_line,
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
    launcher: LauncherEntries,
    jobs: Jobs,
    scroll: f32,
    metrics: Metrics,
    resolves: Resolves,
}

#[derive(Debug, Clone)]
pub enum WindowListMessage {
    Clicked(u64),
    ToggleAudio(i32),
    FaviconsLoaded(HashMap<String, RawFavicon>),
    FallbackFontsResolved(HashMap<char, Font>),
    LauncherEntriesChanged(LauncherEntries),
    JobsChanged(Jobs),
    Tooltip(TooltipMessage),
    IconsBackstopElapsed,
    Scrolled(f32),
    MetricsChanged(Metrics),
}

pub enum Action {
    None,
    FocusWindow(u64),
    ToggleAudio(i32),
    RefreshIcons,
}

impl WindowList {
    pub fn subscription() -> Subscription<WindowListMessage> {
        Subscription::batch([
            every(ICONS_BACKSTOP).map(|_| WindowListMessage::IconsBackstopElapsed),
            launcher_entry::subscription().map(WindowListMessage::LauncherEntriesChanged),
            job_view::subscription().map(WindowListMessage::JobsChanged),
        ])
    }

    pub fn update(&mut self, msg: WindowListMessage) -> Action {
        match msg {
            WindowListMessage::Clicked(window) => Action::FocusWindow(window),
            WindowListMessage::ToggleAudio(pid) => Action::ToggleAudio(pid),
            WindowListMessage::LauncherEntriesChanged(entries) => {
                self.launcher = entries;
                Action::None
            }
            WindowListMessage::JobsChanged(jobs) => {
                self.jobs = jobs;
                Action::None
            }
            WindowListMessage::Tooltip(_) => Action::None,
            WindowListMessage::FaviconsLoaded(raw) => {
                let previous = take(&mut self.favicons);
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
        font: &FontConfig,
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
        font: &FontConfig,
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
        audio: &HashMap<i32, WindowAudio>,
        output: &str,
        config: &Config,
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
            let extras = Extras {
                audio: config
                    .window_list
                    .audio_indicator
                    .then(|| win.pid.and_then(|pid| audio.get(&pid)).copied())
                    .flatten(),
                launcher: self.launcher.get(&win.app_id),
                job: self.jobs.get(&win.app_id),
            };
            window_button(config, icons, win, extras, position, alpha)
        });
        let row = Row::with_children(buttons)
            .spacing(config.spacing.widget_spacing_px * 2.0)
            .align_y(Alignment::Center);
        let focus_alpha = focused_index.map_or(1.0, |i| self.metrics.item_alpha(self.scroll, i));
        let indicated = focus_indicator(row, focused_index)
            .color(config.palette.focus.scale_alpha(focus_alpha))
            .thickness(config.widgets.focus_underline_thickness_px)
            .edge(position);
        Some(window_list_viewport(
            indicated,
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

#[derive(Clone, Copy, Default)]
struct Extras<'a> {
    audio: Option<WindowAudio>,
    launcher: Option<&'a LauncherEntry>,
    job: Option<u8>,
}

impl Extras<'_> {
    fn progress(&self, config: &Config) -> Option<f32> {
        if !config.window_list.progress {
            return None;
        }
        self.job
            .map(|percent| f32::from(percent) / 100.0)
            .or_else(|| self.launcher?.progress)
    }

    fn count(&self, config: &Config) -> Option<i64> {
        config
            .window_list
            .count_badge
            .then(|| self.launcher?.count)
            .flatten()
    }

    fn is_urgent(&self) -> bool {
        self.launcher.is_some_and(|entry| entry.urgent)
    }
}

fn window_button<'a>(
    config: &Config,
    icons: Icons<'a>,
    win: &niri::WinEntry,
    extras: Extras<'_>,
    position: Position,
    alpha: f32,
) -> Element<'a, WindowListMessage> {
    let rule = app_rules::rule_for(&win.app_id);
    let content = button_content(config, icons, rule, win, extras, alpha);
    let indicator = indicator_line(config, win, extras, alpha);
    let slot = PagerSlot::new(content, indicator);
    let slot = match progress_ticks(config, extras, alpha) {
        Some(ticks) => slot.ticks(ticks),
        None => slot,
    };
    let pressable = button(slot.view(config, position))
        .padding(0)
        .style(flat_button(config.palette.bar_foreground))
        .on_press(WindowListMessage::Clicked(win.id));
    let hover = hoverable(
        pressable.into(),
        HoverTarget::Window(win.id),
        WindowListMessage::Tooltip,
    );
    let Some(pid) = win.pid.filter(|_| extras.audio.is_some()) else {
        return hover;
    };
    mouse_area(hover)
        .on_middle_press(WindowListMessage::ToggleAudio(pid))
        .into()
}

fn button_content<'a>(
    config: &Config,
    icons: Icons<'a>,
    rule: AppRule,
    win: &niri::WinEntry,
    extras: Extras<'_>,
    alpha: f32,
) -> Element<'a, WindowListMessage> {
    let icon = window_icon(config, icons, rule, win, extras, alpha);
    let parts = rule.rewrite(label_for(win));
    let title = match rule {
        AppRule::Terminal => title_element(config, &parts, icons.fallbacks, false, alpha),
        AppRule::Firefox | AppRule::Browser { .. } | AppRule::Fallback => {
            title_element(config, &parts, icons.fallbacks, true, alpha)
        }
    };
    Row::new()
        .extend(icon)
        .push(title)
        .extend(audio_glyph(config, extras, alpha))
        .spacing(config.spacing.widget_spacing_px * 1.5)
        .align_y(Alignment::Center)
        .into()
}

fn audio_glyph<'a>(
    config: &Config,
    extras: Extras<'_>,
    alpha: f32,
) -> Option<Element<'a, WindowListMessage>> {
    let audio = extras.audio?;
    if !audio.playing && !audio.muted {
        return None;
    }
    let (icon, color) = if audio.muted {
        (Solid::VolumeXmark, config.palette.inactive)
    } else {
        (Solid::VolumeHigh, config.palette.bar_foreground)
    };
    Some(
        icon.glyph()
            .size(config.font.icon_size_px)
            .color(color.scale_alpha(alpha))
            .into(),
    )
}

fn indicator_line<'a>(
    config: &Config,
    win: &niri::WinEntry,
    extras: Extras<'_>,
    alpha: f32,
) -> Element<'a, WindowListMessage> {
    let opacity = if win.is_urgent || extras.is_urgent() {
        1.0
    } else {
        0.0
    };
    activity_line(
        config.palette.urgent,
        opacity * alpha,
        config.widgets.focus_underline_thickness_px,
    )
}

fn progress_ticks<'a>(
    config: &Config,
    extras: Extras<'_>,
    alpha: f32,
) -> Option<Element<'a, WindowListMessage>> {
    let fraction = extras.progress(config)?;
    Some(progress_line(
        config.palette.warning.scale_alpha(alpha),
        config.palette.dim.scale_alpha(alpha),
        fraction,
        config.widgets.window_segment_thickness_px,
    ))
}

fn count_badge<'a>(
    config: &Config,
    count: i64,
    alpha: f32,
) -> Option<Element<'a, WindowListMessage>> {
    if count <= 0 {
        return None;
    }
    let label = if count > 99 {
        "99+".to_owned()
    } else {
        count.to_string()
    };
    let background = config.palette.urgent.scale_alpha(alpha);
    let foreground = config.palette.bar_background.scale_alpha(alpha);
    let size = config.font.text_size_px;
    let badge = container(text(label).size(size).color(foreground))
        .padding([0.0, config.spacing.widget_spacing_px])
        .style(move |_| container::Style {
            background: Some(background.into()),
            border: iced::Border {
                radius: Radius::new(size),
                ..iced::Border::default()
            },
            ..container::Style::default()
        });
    Some(
        container(badge)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Right)
            .align_y(Vertical::Top)
            .into(),
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
    config: &Config,
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
    config: &Config,
    icons: Icons<'a>,
    rule: AppRule,
    win: &niri::WinEntry,
    extras: Extras<'_>,
    alpha: f32,
) -> Option<Element<'a, WindowListMessage>> {
    let icon = resolved_icon(config, icons, rule, win, alpha)?;
    let badge = extras
        .count(config)
        .and_then(|count| count_badge(config, count, alpha));
    Some(match badge {
        Some(badge) => stack([icon, badge]).into(),
        None => icon,
    })
}

fn resolved_icon<'a>(
    config: &Config,
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
