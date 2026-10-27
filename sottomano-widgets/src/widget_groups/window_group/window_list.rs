use std::collections::HashMap;

use iced::{
    Alignment, Border, Element, Subscription, Task, border,
    widget::{Row, container, progress_bar, right, stack, text},
};
use sottomano_config::{Config, screen_edges::Position, widget_groups::WindowAgentsConfig};
use sottomano_sources::{
    SourceDelivery,
    desktop_windows_apps::{
        kde_job_view_progress::{self, JobProgress},
        kitty_terminal_windows::{self, WindowProcesses},
        niri_window_snapshot,
        nix_package_facts::{NixPackageFacts, PackageWatch},
        process_tree_inspection::{Inspection, Process, Watched},
        unity_launcher_badges::{self, LauncherEntries, LauncherEntry},
    },
    media_playback_audio::pipewire_audio::WindowAudio,
    stream_operators::restarting,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    bar_slot_drag::{DragMessage, SlotClick, SlotDragEvent, SlotPress},
    clock::Clock,
    hover_tooltips::{PressHints, TooltipMessage, hoverable},
    widget_groups::window_group::{WindowTarget, process_tree::Folding},
    widgets::{
        DISABLED_ALPHA,
        activity_line::activity_line,
        bar_widget::Glyph,
        focus_indicator::focus_indicator,
        on_hover_reading::OnHoverReading,
        pager_slot::PagerSlot,
        palette_colors::PaletteColors,
        panel_frame::panel_frame,
        phosphor::{Phosphor, Weight},
        pointer_drag::{
            BarSlotList, DragView, DraggedBarSlot,
            draggable_slot::{draggable_slot, draggable_slot_list},
            keyed_slot_list::keyed_slot_list,
        },
        text::FontFaces,
        tooltip_body::{Section, SourcedSections},
        widget_state::WidgetState,
        window_icon::{WindowAssets, window_icon},
        window_list_viewport::{Metrics, window_list_viewport},
        window_title::WindowTitle,
    },
};

#[derive(Default)]
pub struct WindowList {
    launcher: LauncherEntries,
    jobs: JobProgress,
    scroll: f32,
    metrics: Metrics,
    window_processes: WindowProcesses,
    folding: Folding,
    hovered_window: Option<u64>,
    process_tree: OnHoverReading<Process>,
    inspection: OnHoverReading<Inspection>,
    package_facts: OnHoverReading<NixPackageFacts>,
}

#[derive(Debug, Clone)]
pub enum WindowListMessage {
    ToggleAudio(i32),
    LauncherEntriesChanged(LauncherEntries),
    JobsChanged(JobProgress),
    Tooltip(TooltipMessage),
    ProcessTreeScanned(SourceDelivery<Process>),
    WindowProcessesPaired(WindowProcesses),
    ProcessFoldToggled(i32),
    ProcessInspected(Box<SourceDelivery<Inspection>>),
    PackageFactsRead(Box<SourceDelivery<NixPackageFacts>>),
    Scrolled(f32),
    MetricsChanged(Metrics),
    Drag(DragMessage),
}

pub enum Action {
    None,
    ToggleAudio(i32),
    WindowProcessesPaired,
    Drag(DragMessage),
}

impl WindowList {
    pub fn subscription(agents: &WindowAgentsConfig) -> Subscription<WindowListMessage> {
        Subscription::batch([
            Subscription::run(|| restarting("job view", kde_job_view_progress::serve))
                .map(WindowListMessage::JobsChanged),
            if agents.kitty {
                Subscription::run_with(agents.clone(), |_| {
                    kitty_terminal_windows::window_pairings()
                })
                .map(WindowListMessage::WindowProcessesPaired)
            } else {
                Subscription::none()
            },
        ])
    }

    pub fn launcher_entry_subscription(buses: &DesktopBuses) -> Subscription<WindowListMessage> {
        Subscription::run_with(buses.clone(), unity_launcher_badges::entry_updates)
            .map(WindowListMessage::LauncherEntriesChanged)
    }

    pub fn hover_process(
        &mut self,
        watched: Option<Watched>,
        config: &Config,
        clock: Clock,
    ) -> Task<WindowListMessage> {
        let Some(watched) = watched else {
            self.hovered_window = None;
            self.process_tree.stop();
            self.inspection.stop();
            self.package_facts.stop();
            return Task::none();
        };
        self.hovered_window = Some(watched.window);
        Task::batch([
            self.process_tree.start_hover_fetch(
                watched.clone(),
                WindowListMessage::ProcessTreeScanned,
                config,
                clock,
            ),
            self.inspection.start_hover_fetch(
                watched.clone(),
                |fetched| WindowListMessage::ProcessInspected(Box::new(fetched)),
                config,
                clock,
            ),
            self.package_facts.start_hover_fetch(
                PackageWatch {
                    watched,
                    upstream_branch: config.widget_groups.window_list.nixpkgs_branch.clone(),
                },
                |fetched| WindowListMessage::PackageFactsRead(Box::new(fetched)),
                config,
                clock,
            ),
        ])
    }

    #[must_use]
    pub fn window_processes(&self) -> &WindowProcesses {
        &self.window_processes
    }

    #[must_use]
    pub fn process_sections(&self, clock: Clock) -> Vec<SourcedSections> {
        self.hovered_window
            .iter()
            .flat_map(|window| {
                [
                    self.inspection.sections(clock),
                    self.package_facts.sections(clock),
                    self.process_tree.sections_described_by(|tree| {
                        let kept = self
                            .window_processes
                            .of_window(*window)
                            .map_or_else(|| tree.clone(), |kept| tree.pruned_to(kept));
                        vec![Section::tree(Some(&kept), &self.folding).titled("Process")]
                    }),
                ]
                .into_iter()
                .flatten()
            })
            .collect()
    }

    pub fn update(&mut self, msg: WindowListMessage, clock: Clock) -> Action {
        match msg {
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
                self.process_tree.receive_delivery(fetched, clock);
                Action::None
            }
            WindowListMessage::WindowProcessesPaired(window_processes) => {
                self.window_processes = window_processes;
                Action::WindowProcessesPaired
            }
            WindowListMessage::ProcessFoldToggled(pid) => {
                self.folding.toggle(pid);
                Action::None
            }
            WindowListMessage::ProcessInspected(fetched) => {
                self.inspection.receive_delivery(*fetched, clock);
                Action::None
            }
            WindowListMessage::PackageFactsRead(fetched) => {
                self.package_facts.receive_delivery(*fetched, clock);
                Action::None
            }
            WindowListMessage::Scrolled(offset) => {
                self.scroll = offset.clamp(0.0, self.metrics.max_scroll());
                Action::None
            }
            WindowListMessage::MetricsChanged(metrics) => {
                self.metrics = metrics;
                self.scroll = self.scroll.clamp(0.0, self.metrics.max_scroll());
                Action::None
            }
            WindowListMessage::Drag(drag) => Action::Drag(drag),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn view<'a>(
        &'a self,
        niri: &niri_window_snapshot::Snapshot,
        icons: WindowAssets<'a>,
        audio: Option<&HashMap<i32, WindowAudio>>,
        output: &str,
        config: &Config,
        position: Position,
        drag: &DragView,
    ) -> Option<Element<'a, WindowListMessage>> {
        let workspace = drag
            .window_source_workspace()
            .filter(|source| {
                niri.workspaces_for_output(output)
                    .iter()
                    .any(|indicator| indicator.id == *source)
            })
            .or_else(|| niri.active_workspace_id(output))?;
        let windows = niri.windows_on_workspace(workspace);
        if windows.is_empty() {
            return None;
        }
        let dragged_item = |win: &niri_window_snapshot::WinEntry| DraggedBarSlot::Window {
            window: win.id,
            workspace,
        };
        let order = drag.previewed(windows, dragged_item);
        let focused_index = order.iter().position(|win| win.is_focused);
        let slots = order.iter().enumerate().map(|(position_in_bar, win)| {
            let item = dragged_item(win);
            let dimming = if drag.is_dragging(item) {
                DISABLED_ALPHA
            } else {
                1.0
            };
            let opacity = self.metrics.item_opacity(self.scroll, position_in_bar) * dimming;
            let slot = WindowSlot {
                win,
                item,
                extras: self.extras(win, audio, config),
                position_in_bar,
                opacity,
            };
            (item, slot.view(config, icons, position, drag))
        });
        let focus_opacity =
            focused_index.map_or(1.0, |i| self.metrics.item_opacity(self.scroll, i));
        let indicated = focus_indicator(
            keyed_slot_list(slots, position, config),
            focused_index,
            PaletteColors::from(&config.theme.palette)
                .focus
                .scale_alpha(focus_opacity),
            config,
        )
        .edge(position);
        Some(window_list_viewport(
            draggable_slot_list(indicated, BarSlotList::Windows, drag, dragged),
            self.scroll,
            focused_index,
            WindowListMessage::Scrolled,
            WindowListMessage::MetricsChanged,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn drag_ghost<'a>(
        &'a self,
        niri: &niri_window_snapshot::Snapshot,
        icons: WindowAssets<'a>,
        audio: Option<&HashMap<i32, WindowAudio>>,
        window: u64,
        config: &Config,
        drag: &DragView,
    ) -> Option<Element<'a, WindowListMessage>> {
        let win = niri
            .windows_on_workspace(drag.window_source_workspace()?)
            .iter()
            .find(|win| win.id == window)?;
        if drag.has_left_its_list() {
            let icon = resolved_icon(config, icons, win, 1.0)?;
            let inset = config.layout.overlay.content_inset;
            let circle = container::Style {
                border: Border {
                    radius: (config.theme.font.icon_size_px / 2.0 + inset).into(),
                    ..drag.ghost_frame.border
                },
                ..drag.ghost_frame
            };
            return Some(panel_frame(container(icon).padding(inset), &circle).into());
        }
        let content = button_content(config, icons, win, self.extras(win, audio, config), 1.0);
        Some(
            panel_frame(
                container(content)
                    .padding([0.0, config.layout.panel.group_padding])
                    .center_y(config.layout.panel.height),
                &drag.ghost_frame,
            )
            .into(),
        )
    }

    fn extras(
        &self,
        win: &niri_window_snapshot::WinEntry,
        audio: Option<&HashMap<i32, WindowAudio>>,
        config: &Config,
    ) -> Extras<'_> {
        Extras {
            audio: config
                .widget_groups
                .window_list
                .audio_indicator
                .then(|| win.pid.and_then(|pid| audio?.get(&pid)).copied())
                .flatten(),
            launcher: win.app_id.as_deref().and_then(|id| self.launcher.get(id)),
            job: win.app_id.as_deref().and_then(|id| self.jobs.get(id)),
        }
    }
}

fn dragged(event: SlotDragEvent) -> WindowListMessage {
    WindowListMessage::Drag(DragMessage::Pointer(event))
}

struct WindowSlot<'w, 'e> {
    win: &'w niri_window_snapshot::WinEntry,
    item: DraggedBarSlot,
    extras: Extras<'e>,
    position_in_bar: usize,
    opacity: f32,
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

impl WindowSlot<'_, '_> {
    fn view<'a>(
        self,
        config: &Config,
        icons: WindowAssets<'a>,
        position: Position,
        drag: &DragView,
    ) -> Element<'a, WindowListMessage> {
        let WindowSlot {
            win,
            item,
            extras,
            position_in_bar,
            opacity,
        } = self;
        let maximize_toggles = win.is_focused && !win.layout.is_floating;
        let press = SlotPress {
            item,
            position: position_in_bar,
            click: if maximize_toggles {
                SlotClick::MaximizeColumn
            } else {
                SlotClick::FocusWindow(win.id)
            },
        };
        let tiled = win.layout.column.is_some() && !win.layout.is_floating;
        let pressable = draggable_slot(
            window_slot_content(config, icons, win, extras, position, opacity),
            press,
            tiled,
            drag,
            dragged,
        );
        match win.pid.filter(|_| extras.audio.is_some()) {
            Some(pid) => pressable
                .on_middle_press(WindowListMessage::ToggleAudio(pid))
                .into(),
            None => pressable.into(),
        }
    }
}

fn window_slot_content<'a>(
    config: &Config,
    icons: WindowAssets<'a>,
    win: &niri_window_snapshot::WinEntry,
    extras: Extras<'_>,
    position: Position,
    opacity: f32,
) -> Element<'a, WindowListMessage> {
    let maximize_toggles = win.is_focused && !win.layout.is_floating;
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
    let picking = hoverable(
        button_content(config, icons, win, extras, opacity),
        WindowTarget::Pick(win.id).into(),
        press_hints,
        WindowListMessage::Tooltip,
    );
    let usage = win.is_focused.then(|| {
        hoverable(
            secondary_glyph(config, Phosphor::UserFocus, opacity),
            WindowTarget::Usage(win.id).into(),
            press_hints,
            WindowListMessage::Tooltip,
        )
    });
    let inspecting = win.is_focused.then(|| {
        hoverable(
            secondary_glyph(config, Phosphor::Package, opacity),
            WindowTarget::Inspect(win.id).into(),
            press_hints,
            WindowListMessage::Tooltip,
        )
    });
    let content = Row::new()
        .push(picking)
        .extend(usage)
        .extend(inspecting)
        .spacing(config.layout.panel.widget_spacing)
        .align_y(Alignment::Center);
    let indicator = indicator_line(config, win, extras, opacity);
    let slot =
        PagerSlot::new(content.into(), indicator).spaced_by(config.layout.panel.group_spacing);
    let slot = match progress_ticks(config, extras, opacity) {
        Some(ticks) => slot.ticks(ticks),
        None => slot,
    };
    slot.view(config, position)
}

fn secondary_glyph<'a>(
    config: &Config,
    icon: fn(Weight) -> Phosphor,
    opacity: f32,
) -> Element<'a, WindowListMessage> {
    let state = WidgetState::Off;
    let color = state
        .color(&PaletteColors::from(&config.theme.palette))
        .scale_alpha(opacity);
    Glyph::from(icon(state.weight())).view(config.theme.font.icon_size_px, color)
}

fn button_content<'a>(
    config: &Config,
    icons: WindowAssets<'a>,
    win: &niri_window_snapshot::WinEntry,
    extras: Extras<'_>,
    opacity: f32,
) -> Element<'a, WindowListMessage> {
    let icon = badged_icon(config, icons, win, extras, opacity);
    let title = win
        .label
        .as_ref()
        .map(|label| WindowTitle::new(label, config).opacity(opacity));
    Row::new()
        .extend(icon)
        .extend(title.map(Element::from))
        .extend(audio_glyph(config, extras, opacity))
        .spacing(config.layout.panel.widget_spacing)
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
    let color = state
        .color(&PaletteColors::from(&config.theme.palette))
        .scale_alpha(opacity);
    Some(Glyph::from(icon.with_weight(state.weight())).view(config.theme.font.icon_size_px, color))
}

fn indicator_line<'a>(
    config: &Config,
    win: &niri_window_snapshot::WinEntry,
    extras: Extras<'_>,
    opacity: f32,
) -> Element<'a, WindowListMessage> {
    let severity = if win.is_urgent || extras.is_urgent() {
        1.0
    } else {
        0.0
    };
    activity_line(
        PaletteColors::from(&config.theme.palette).urgent,
        severity * opacity,
        config.panel_bar.indicator_thickness,
    )
}

fn progress_ticks<'a>(
    config: &Config,
    extras: Extras<'_>,
    opacity: f32,
) -> Option<Element<'a, WindowListMessage>> {
    let fraction = extras.progress(config)?;
    let thickness = config.panel_bar.indicator_thickness;
    let filled = PaletteColors::from(&config.theme.palette)
        .warning
        .scale_alpha(opacity);
    let track = PaletteColors::from(&config.theme.palette)
        .dim
        .scale_alpha(opacity);
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
    let background = PaletteColors::from(&config.theme.palette)
        .urgent
        .scale_alpha(opacity);
    let foreground = PaletteColors::from(&config.theme.palette)
        .surface
        .scale_alpha(opacity);
    let size = config.theme.font.text_size_px;
    let badge = container(
        text(label)
            .size(size)
            .font(FontFaces::from(&config.theme.font).regular)
            .color(foreground),
    )
    .padding([0.0, config.layout.panel.widget_spacing])
    .style(move |_| {
        container::Style::default()
            .background(background)
            .border(border::rounded(size))
    });
    Some(right(badge).into())
}

fn badged_icon<'a>(
    config: &Config,
    icons: WindowAssets<'a>,
    win: &niri_window_snapshot::WinEntry,
    extras: Extras<'_>,
    opacity: f32,
) -> Option<Element<'a, WindowListMessage>> {
    let icon = resolved_icon(config, icons, win, opacity)?;
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
    win: &niri_window_snapshot::WinEntry,
    opacity: f32,
) -> Option<Element<'a, WindowListMessage>> {
    let path = icons.path_for(win);
    window_icon(
        icons.favicons,
        win.favicon_key(),
        path.as_deref(),
        config.theme.font.icon_size_px,
        opacity,
    )
}
