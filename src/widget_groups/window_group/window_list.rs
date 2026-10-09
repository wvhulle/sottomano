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
        app_rules::AppRule,
        audio::WindowAudio,
        job_view::{self, Jobs},
        launcher_entry::{self, LauncherEntries, LauncherEntry},
        niri,
        process::{self, Forest, Inspection, Process},
    },
    desktop_shell::{
        hover_tooltips::{HoverTarget, PressHints, TooltipMessage, hoverable},
        monitor_surfaces::Position,
    },
    widget_groups::window_group::process_tree::Folding,
    widgets::{
        activity_line::activity_line,
        bar_widget::Glyph,
        flat_button,
        focus_indicator::focus_indicator,
        pager_slot::PagerSlot,
        phosphor::{Phosphor, Weight},
        progress_line::progress_line,
        text::LINE_HEIGHT,
        widget_state::WidgetState,
        window_icon::{WindowAssets, window_icon},
        window_list_viewport::{Metrics, window_list_viewport},
        window_title::{title_runs, title_spans},
    },
};

const ICONS_BACKSTOP: Duration = Duration::from_secs(20);

#[derive(Default)]
pub struct WindowList {
    launcher: LauncherEntries,
    jobs: Jobs,
    scroll: f32,
    metrics: Metrics,
    processes: Forest,
    folding: Folding,
    inspected: Option<Inspection>,
}

#[derive(Debug, Clone)]
pub enum WindowListMessage {
    Clicked(u64),
    MaximizeToggled,
    ToggleAudio(i32),
    LauncherEntriesChanged(LauncherEntries),
    JobsChanged(Jobs),
    Tooltip(TooltipMessage),
    ProcessesScanned(Forest),
    ProcessFoldToggled(i32),
    ProcessInspected(Inspection),
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
    pub fn subscription(watching_window: bool) -> Subscription<WindowListMessage> {
        let watched = if watching_window {
            process::subscription().map(WindowListMessage::ProcessesScanned)
        } else {
            Subscription::none()
        };
        Subscription::batch([
            every(ICONS_BACKSTOP).map(|_| WindowListMessage::IconsBackstopElapsed),
            launcher_entry::subscription().map(WindowListMessage::LauncherEntriesChanged),
            job_view::subscription().map(WindowListMessage::JobsChanged),
            watched,
        ])
    }

    pub fn process(&self, pid: i32) -> Option<&Process> {
        self.processes.subtree(pid)
    }

    pub fn folding(&self) -> &Folding {
        &self.folding
    }

    pub fn inspection(&self, pid: i32) -> Option<&Inspection> {
        self.inspected.as_ref().filter(|found| found.pid == pid)
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
            WindowListMessage::ProcessesScanned(forest) => {
                self.folding.retain_live(&forest);
                self.processes = forest;
                Action::None
            }
            WindowListMessage::ProcessFoldToggled(pid) => {
                self.folding.toggle(pid);
                Action::None
            }
            WindowListMessage::ProcessInspected(inspected) => {
                self.inspected = Some(inspected);
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

    pub fn view<'a>(
        &'a self,
        niri: &niri::Snapshot,
        icons: WindowAssets<'a>,
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
                launcher: win.app_id.as_deref().and_then(|id| self.launcher.get(id)),
                job: win.app_id.as_deref().and_then(|id| self.jobs.get(id)),
            };
            window_button(config, icons, win, extras, position, opacity)
        });
        let row = Row::with_children(buttons)
            .spacing(config.spacing.group_spacing_px)
            .height(Length::Fill)
            .align_y(Alignment::Center);
        let focus_opacity =
            focused_index.map_or(1.0, |i| self.metrics.item_opacity(self.scroll, i));
        let indicated = focus_indicator(
            row,
            focused_index,
            config.palette.focus.scale_alpha(focus_opacity),
        )
        .thickness(config.widget_sizes.indicator_thickness_px)
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
    icons: WindowAssets<'a>,
    win: &niri::WinEntry,
    extras: Extras<'_>,
    position: Position,
    opacity: f32,
) -> Element<'a, WindowListMessage> {
    let rule = win.app_rule();
    let content = button_content(config, icons, rule, win, extras, opacity);
    let indicator = indicator_line(config, win, extras, opacity);
    let slot = PagerSlot::new(content, indicator);
    let slot = match progress_ticks(config, extras, opacity) {
        Some(ticks) => slot.ticks(ticks),
        None => slot,
    };
    let maximize_toggles = win.is_focused && !win.layout.is_floating;
    let pressable = button(slot.view(config, position))
        .padding(0)
        .style(flat_button(config.palette.bar_foreground))
        .on_press(if maximize_toggles {
            WindowListMessage::MaximizeToggled
        } else {
            WindowListMessage::Clicked(win.id)
        });
    let audio_pid = win.pid.filter(|_| extras.audio.is_some());
    let press_hints = PressHints {
        left: Some(if maximize_toggles {
            "maximize or restore"
        } else {
            "focus the window"
        }),
        middle: audio_pid.map(|_| "mute or unmute its audio"),
        ..PressHints::default()
    };
    let hover = hoverable(
        pressable.into(),
        HoverTarget::Window(win.id),
        press_hints,
        WindowListMessage::Tooltip,
    );
    let Some(pid) = audio_pid else {
        return hover;
    };
    mouse_area(hover)
        .on_middle_press(WindowListMessage::ToggleAudio(pid))
        .into()
}

fn button_content<'a>(
    config: &Config,
    icons: WindowAssets<'a>,
    rule: AppRule,
    win: &niri::WinEntry,
    extras: Extras<'_>,
    opacity: f32,
) -> Element<'a, WindowListMessage> {
    let icon = badged_icon(config, icons, rule, win, extras, opacity);
    let runs = title_runs(
        &config.font,
        icons.fallbacks,
        win.title_parts.as_ref(),
        win.lead_first,
    );
    let title = rich_text(title_spans(config, &runs, opacity))
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
    let state = WidgetState::of_toggle(!audio.muted);
    let icon = if audio.muted {
        Phosphor::SpeakerX(Weight::Regular)
    } else {
        Phosphor::SpeakerHigh(Weight::Regular)
    };
    let color = state.color(&config.palette).scale_alpha(opacity);
    Some(Glyph::from(icon.with_weight(state.weight())).view(config.font.icon_size_px, color))
}

fn indicator_line<'a>(
    config: &Config,
    win: &niri::WinEntry,
    extras: Extras<'_>,
    opacity: f32,
) -> Element<'a, WindowListMessage> {
    let severity = if win.is_urgent || extras.is_urgent() {
        1.0
    } else {
        0.0
    };
    activity_line(
        config.palette.urgent,
        severity * opacity,
        config.widget_sizes.indicator_thickness_px,
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
        config.widget_sizes.indicator_thickness_px,
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
    let foreground = config.palette.surface.scale_alpha(opacity);
    let size = config.font.text_size_px;
    let badge = container(
        text(label)
            .size(size)
            .font(config.font.iced_font)
            .color(foreground),
    )
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
    icons: WindowAssets<'a>,
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
    icons: WindowAssets<'a>,
    rule: AppRule,
    win: &niri::WinEntry,
    opacity: f32,
) -> Option<Element<'a, WindowListMessage>> {
    let path = icons.path_for(rule, win);
    window_icon(
        icons.favicons,
        win.title
            .as_deref()
            .and_then(|title| rule.favicon_key(title))
            .as_deref(),
        path.as_deref(),
        config.font.icon_size_px,
        opacity,
    )
}
