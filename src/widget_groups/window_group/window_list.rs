use std::{collections::HashMap, time::Duration};

use iced::{
    Alignment, Element, Length, Subscription,
    alignment::{Horizontal, Vertical},
    border::Radius,
    time::every,
    widget::{Row, button, container, mouse_area, rich_text, stack, text},
};

use crate::{
    config::Config,
    data::source::{
        app_rules::{self, AppRule},
        audio::WindowAudio,
        font_awesome::Solid,
        job_view::{self, Jobs},
        launcher_entry::{self, LauncherEntries, LauncherEntry},
        niri,
    },
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
        text::LINE_HEIGHT,
        window_icon::{WindowIcons, window_icon},
        window_list_viewport::{Metrics, window_list_viewport},
        window_title::title_spans,
    },
};

const ICONS_BACKSTOP: Duration = Duration::from_secs(20);

#[derive(Default)]
pub struct WindowList {
    launcher: LauncherEntries,
    jobs: Jobs,
    scroll: f32,
    metrics: Metrics,
}

#[derive(Debug, Clone)]
pub enum WindowListMessage {
    Clicked(u64),
    MaximizeToggled,
    ToggleAudio(i32),
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
    MaximizeColumn,
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
            WindowListMessage::MaximizeToggled => Action::MaximizeColumn,
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

    pub fn view<'a>(
        &'a self,
        niri: &niri::Snapshot,
        icons: WindowIcons<'a>,
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
        let focused_index = windows.iter().position(|win| win.is_focused);
        let buttons = windows.iter().enumerate().map(|(i, win)| {
            let opacity = self.metrics.item_opacity(self.scroll, i);
            let extras = Extras {
                audio: config
                    .window_list
                    .audio_indicator
                    .then(|| win.pid.and_then(|pid| audio.get(&pid)).copied())
                    .flatten(),
                launcher: self.launcher.get(&win.app_id),
                job: self.jobs.get(&win.app_id),
            };
            window_button(config, icons, win, extras, position, opacity)
        });
        let row = Row::with_children(buttons)
            .spacing(config.spacing.group_spacing_px)
            .align_y(Alignment::Center);
        let focus_opacity =
            focused_index.map_or(1.0, |i| self.metrics.item_opacity(self.scroll, i));
        let indicated = focus_indicator(row, focused_index)
            .color(config.palette.focus.scale_alpha(focus_opacity))
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
    icons: WindowIcons<'a>,
    win: &niri::WinEntry,
    extras: Extras<'_>,
    position: Position,
    opacity: f32,
) -> Element<'a, WindowListMessage> {
    let rule = app_rules::rule_for(&win.app_id);
    let content = button_content(config, icons, rule, win, extras, opacity);
    let indicator = indicator_line(config, win, extras, opacity);
    let slot = PagerSlot::new(content, indicator);
    let slot = match progress_ticks(config, extras, opacity) {
        Some(ticks) => slot.ticks(ticks),
        None => slot,
    };
    let pressable = button(slot.view(config, position))
        .padding(0)
        .style(flat_button(config.palette.bar_foreground))
        .on_press(if win.is_focused && !win.layout.is_floating {
            WindowListMessage::MaximizeToggled
        } else {
            WindowListMessage::Clicked(win.id)
        });
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
    icons: WindowIcons<'a>,
    rule: AppRule,
    win: &niri::WinEntry,
    extras: Extras<'_>,
    opacity: f32,
) -> Element<'a, WindowListMessage> {
    let icon = badged_icon(config, icons, rule, win, extras, opacity);
    let title = rich_text(title_spans(config, &win.title_runs, opacity))
        .size(config.font.text_size_px)
        .line_height(LINE_HEIGHT);
    Row::new()
        .extend(icon)
        .push(title)
        .extend(audio_glyph(config, extras, opacity))
        .spacing(config.spacing.widget_spacing_px)
        .align_y(Alignment::Center)
        .into()
}

fn audio_glyph<'a>(
    config: &Config,
    extras: Extras<'_>,
    opacity: f32,
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
            .color(color.scale_alpha(opacity))
            .into(),
    )
}

fn indicator_line<'a>(
    config: &Config,
    win: &niri::WinEntry,
    extras: Extras<'_>,
    opacity: f32,
) -> Element<'a, WindowListMessage> {
    let urgency = if win.is_urgent || extras.is_urgent() {
        1.0
    } else {
        0.0
    };
    activity_line(
        config.palette.urgent,
        urgency * opacity,
        config.widgets.focus_underline_thickness_px,
    )
}

fn progress_ticks<'a>(
    config: &Config,
    extras: Extras<'_>,
    opacity: f32,
) -> Option<Element<'a, WindowListMessage>> {
    let fraction = extras.progress(config)?;
    Some(progress_line(
        config.palette.warning.scale_alpha(opacity),
        config.palette.dim.scale_alpha(opacity),
        fraction,
        config.widgets.window_segment_thickness_px,
    ))
}

fn count_badge<'a>(
    config: &Config,
    count: i64,
    opacity: f32,
) -> Option<Element<'a, WindowListMessage>> {
    if count <= 0 {
        return None;
    }
    let label = if count > 99 {
        "99+".to_owned()
    } else {
        count.to_string()
    };
    let background = config.palette.urgent.scale_alpha(opacity);
    let foreground = config.palette.bar_background.scale_alpha(opacity);
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

fn badged_icon<'a>(
    config: &Config,
    icons: WindowIcons<'a>,
    rule: AppRule,
    win: &niri::WinEntry,
    extras: Extras<'_>,
    opacity: f32,
) -> Option<Element<'a, WindowListMessage>> {
    let icon = resolved_icon(config, icons, rule, win, opacity)?;
    let badge = extras
        .count(config)
        .and_then(|count| count_badge(config, count, opacity));
    Some(match badge {
        Some(badge) => stack([icon, badge]).into(),
        None => icon,
    })
}

fn resolved_icon<'a>(
    config: &Config,
    icons: WindowIcons<'a>,
    rule: AppRule,
    win: &niri::WinEntry,
    opacity: f32,
) -> Option<Element<'a, WindowListMessage>> {
    let path = icons.path_for(rule, win);
    window_icon(
        icons.favicons,
        rule.favicon_key(&win.title).as_deref(),
        path.as_deref(),
        config.font.icon_size_px,
        opacity,
    )
}
