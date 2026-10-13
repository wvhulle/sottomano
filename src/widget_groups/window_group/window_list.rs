use std::{collections::HashMap, time::Duration};

use iced::{
    Alignment, Element, Length, Subscription, Task,
    alignment::{Horizontal, Vertical},
    border,
    border::Radius,
    time::every,
    widget::{Row, button, container, mouse_area, progress_bar, rich_text, stack, text},
};

use crate::{
    config::{Config, widget_groups::WindowAgentsConfig},
    data::source::{
        Delivery,
        app_rules::AppRule,
        audio::WindowAudio,
        job_view::{self, Jobs},
        launcher_entry::{self, LauncherEntries, LauncherEntry},
        niri,
        process::{Inspection, Process, Watched},
        window_agents::{self, WindowProcesses},
    },
    desktop_shell::{
        hover_tooltips::{HoverTarget, PressHints, TooltipMessage, hoverable},
        monitor_surfaces::Position,
    },
    widget_groups::window_group::{process_tree::Folding, window_tooltip},
    widgets::{
        activity_line::activity_line,
        bar_widget::Glyph,
        flat_button,
        focus_indicator::focus_indicator,
        on_hover_reading::OnHoverReading,
        pager_slot::PagerSlot,
        phosphor::{Phosphor, Weight},
        text::LINE_HEIGHT,
        tooltip_body::{Section, SourcedSection},
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
    window_processes: WindowProcesses,
    folding: Folding,
    hovered_window: Option<u64>,
    process_tree: OnHoverReading<Process>,
    inspection: OnHoverReading<Inspection>,
}

#[derive(Debug, Clone)]
pub enum WindowListMessage {
    Clicked(u64),
    MaximizeToggled,
    ToggleAudio(i32),
    LauncherEntriesChanged(LauncherEntries),
    JobsChanged(Jobs),
    Tooltip(TooltipMessage),
    ProcessTreeScanned(Delivery<Process>),
    WindowProcessesPaired(WindowProcesses),
    ProcessFoldToggled(i32),
    ProcessInspected(Box<Delivery<Inspection>>),
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
    pub fn subscription(agents: &WindowAgentsConfig) -> Subscription<WindowListMessage> {
        Subscription::batch([
            every(ICONS_BACKSTOP).map(|_| WindowListMessage::IconsBackstopElapsed),
            launcher_entry::subscription().map(WindowListMessage::LauncherEntriesChanged),
            job_view::subscription().map(WindowListMessage::JobsChanged),
            window_agents::subscription(agents).map(WindowListMessage::WindowProcessesPaired),
        ])
    }

    pub fn hover_process(
        &mut self,
        watched: Option<Watched>,
        config: &Config,
    ) -> Task<WindowListMessage> {
        let Some(watched) = watched else {
            self.hovered_window = None;
            self.process_tree.stop();
            self.inspection.stop();
            return Task::none();
        };
        self.hovered_window = Some(watched.window);
        Task::batch([
            self.process_tree.poll(
                watched.clone(),
                WindowListMessage::ProcessTreeScanned,
                config,
            ),
            self.inspection.poll(
                watched,
                |fetched| WindowListMessage::ProcessInspected(Box::new(fetched)),
                config,
            ),
        ])
    }

    pub fn process_sections(&self) -> Vec<SourcedSection> {
        self.hovered_window
            .iter()
            .flat_map(|window| {
                [
                    self.inspection
                        .sections(window_tooltip::inspection_sections),
                    self.process_tree.sections(|tree| {
                        let kept = self
                            .window_processes
                            .of_window(*window)
                            .map_or_else(|| tree.clone(), |kept| tree.pruned_to(kept));
                        vec![Section::tree(Some(&kept), &self.folding).titled("Process")]
                    }),
                ]
                .concat()
            })
            .collect()
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
            WindowListMessage::ProcessTreeScanned(fetched) => {
                self.process_tree.receive(fetched);
                Action::None
            }
            WindowListMessage::WindowProcessesPaired(window_processes) => {
                self.window_processes = window_processes;
                Action::None
            }
            WindowListMessage::ProcessFoldToggled(pid) => {
                self.folding.toggle(pid);
                Action::None
            }
            WindowListMessage::ProcessInspected(fetched) => {
                self.inspection.receive(*fetched);
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
        audio: Option<&HashMap<i32, WindowAudio>>,
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
                    .widget_groups
                    .window_list
                    .audio_indicator
                    .then(|| win.pid.and_then(|pid| audio?.get(&pid)).copied())
                    .flatten(),
                launcher: win.app_id.as_deref().and_then(|id| self.launcher.get(id)),
                job: win.app_id.as_deref().and_then(|id| self.jobs.get(id)),
            };
            window_button(config, icons, win, extras, position, opacity)
        });
        let row = Row::with_children(buttons)
            .height(Length::Fill)
            .align_y(Alignment::Center);
        let focus_opacity =
            focused_index.map_or(1.0, |i| self.metrics.item_opacity(self.scroll, i));
        let indicated = focus_indicator(
            row,
            focused_index,
            config.theme.palette.focus.scale_alpha(focus_opacity),
            config,
        )
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
        if !config.widget_groups.window_list.progress {
            return None;
        }
        self.job
            .map(|percent| f32::from(percent) / 100.0)
            .or_else(|| self.launcher?.progress)
    }

    fn count(&self, config: &Config) -> Option<i64> {
        config
            .widget_groups
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
    let slot = PagerSlot::new(content, indicator).spaced_by(config.panel_bar.group_spacing_px);
    let slot = match progress_ticks(config, extras, opacity) {
        Some(ticks) => slot.ticks(ticks),
        None => slot,
    };
    let maximize_toggles = win.is_focused && !win.layout.is_floating;
    let pressable = button(slot.view(config, position))
        .padding(0)
        .style(flat_button(config.theme.palette.bar_foreground))
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
        &config.theme.font,
        icons.fallbacks,
        win.title_parts.as_ref(),
        win.lead_first,
    );
    let title = rich_text(title_spans(config, &runs, opacity))
        .size(config.theme.font.text_size_px)
        .line_height(LINE_HEIGHT);
    Row::new()
        .extend(icon)
        .push(title)
        .extend(audio_glyph(config, extras, opacity))
        .spacing(config.panel_bar.widget_spacing_px)
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
    let color = state.color(&config.theme.palette).scale_alpha(opacity);
    Some(Glyph::from(icon.with_weight(state.weight())).view(config.theme.font.icon_size_px, color))
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
        config.theme.palette.urgent,
        severity * opacity,
        config.panel_bar.indicator_thickness_px,
    )
}

fn progress_ticks<'a>(
    config: &Config,
    extras: Extras<'_>,
    opacity: f32,
) -> Option<Element<'a, WindowListMessage>> {
    let fraction = extras.progress(config)?;
    let thickness = config.panel_bar.indicator_thickness_px;
    let filled = config.theme.palette.warning.scale_alpha(opacity);
    let track = config.theme.palette.dim.scale_alpha(opacity);
    Some(
        progress_bar(0.0..=1.0, fraction)
            .girth(thickness)
            .style(move |_| progress_bar::Style {
                background: track.into(),
                bar: filled.into(),
                border: border::rounded(thickness),
            })
            .into(),
    )
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
    let background = config.theme.palette.urgent.scale_alpha(opacity);
    let foreground = config.theme.palette.surface.scale_alpha(opacity);
    let size = config.theme.font.text_size_px;
    let badge = container(
        text(label)
            .size(size)
            .font(config.theme.font.iced_font)
            .color(foreground),
    )
    .padding([0.0, config.panel_bar.widget_spacing_px])
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
        config.theme.font.icon_size_px,
        opacity,
    )
}
