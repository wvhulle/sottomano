mod slot_extras;
#[cfg(test)]
mod window_list_tests;
mod window_slot;

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use corredo::{
    drag_reorder::{ListDeparture, SlotAlarm},
    orientation::Axis,
    selection_scroll::ScrollEdgeFade,
    theming::{danger_color, primary_color},
    transitions::{DepartingPane, MotionState, SlideBand, SwapPane, sliding_pane_swap},
};
use iced::{
    Animation, Element, Subscription, Task,
    animation::Easing,
    widget::{Row, container},
};
use slot_extras::{Extras, WindowSound, slot_press_actions};
use sottomano_compositor_model::compositor_window_facts::{
    Urgency, WindowAppId, WindowFocus, WindowPid,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    desktop_windows_apps::{
        kde_job_view_progress::{self, JobProgress},
        nix_package_facts::{NixPackageFacts, PackageWatch},
        process_tree_inspection::{Inspection, Process, Watched},
        terminal_emulator_windows::{self, WindowProcesses},
        unity_launcher_badges::{self, LauncherCount, LauncherEntries, LauncherProgress},
        workspace_window_snapshot,
    },
    media_playback_audio::pipewire_audio::WindowAudio,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::{
    Config,
    indicator_groups::{ElementVisibility, TerminalAgentUse, WindowAgentsConfig},
    screen_edges::Position,
};
use window_slot::{UrgencyMarking, WindowSlot};

use crate::{
    clock::Clock,
    desktop_panels::{
        panel_frame::panel_frame,
        panel_slot::dragged_slot_content,
        panel_slot_drag::{DragMessage, PanelSlotContext, SlotDragEvent, window_source_workspace},
    },
    hover_cards::{HoverCardMessage, PressAction, hover_card_body::Section},
    icon_catalog::window_icons::WindowAssets,
    indicator_groups::window_group::{
        WindowRowLayout, WindowTitleLength,
        flat_window_rows::arranged_flat_rows,
        keyed_window_group_rows::arranged_keyed_groups,
        process_tree::{Folding, process_tree_widget},
        window_hover_card::{inspection_sections, nix_package_facts_sections},
        window_row_presentation::{ArrangedWindowRows, WindowSlotPresentation},
    },
    widgets::{
        DISABLED_ALPHA,
        configured_scroll::configured_scroll,
        hover_fetched_snapshot::HoverFetchedSnapshot,
        pointer_drag::{
            DragActivity, DragView, DraggedPanelSlot, PanelSlotList,
            reorderable_list::reorderable_list,
        },
    },
};

fn maximum_window_title_width_px(config: &Config) -> f32 {
    config
        .indicator_groups
        .window
        .maximum_window_title_width
        .resolve(config.theme.font.text_size_px)
}

fn capped_window_title<'view, Message: 'view>(
    title: impl Into<Element<'view, Message>>,
    config: &Config,
    window_title_length: WindowTitleLength,
) -> Element<'view, Message> {
    match window_title_length {
        WindowTitleLength::Capped => container(title)
            .max_width(maximum_window_title_width_px(config))
            .clip(true)
            .into(),
        WindowTitleLength::Full => title.into(),
    }
}

#[derive(Debug, Clone, Copy, Default)]
enum ProcessHover {
    #[default]
    Idle,
    Watching(u64),
}

#[derive(Default)]
pub struct WindowList {
    launcher: LauncherEntries,
    jobs: JobProgress,
    pub window_processes: WindowProcesses,
    folding: Folding,
    hovered_window: ProcessHover,
    process_tree: HoverFetchedSnapshot<Process>,
    inspection: HoverFetchedSnapshot<Inspection>,
    package_facts: HoverFetchedSnapshot<NixPackageFacts>,
    output_workspaces: HashMap<String, OutputWorkspaces>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShownWorkspace {
    workspace_id: u64,
    workspace_index: u8,
}

#[derive(Debug, Clone)]
struct DepartureSlide {
    departed: ShownWorkspace,
    animation: Animation<bool>,
}

#[derive(Debug, Clone)]
enum WorkspaceSlide {
    Settled,
    Sliding(DepartureSlide),
}

#[derive(Debug, Clone)]
struct OutputWorkspaces {
    shown: ShownWorkspace,
    workspace_slide: WorkspaceSlide,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum DepartingWorkspace {
    Absent,
    SlidingAway {
        departed: ShownWorkspace,
        slide_progress: f32,
    },
}

fn started_slide(now: Instant, slide_duration: Duration) -> Animation<bool> {
    Animation::new(false)
        .easing(Easing::EaseOutCubic)
        .duration(slide_duration)
        .go(true, now)
}

fn shown_workspace(
    compositor: &workspace_window_snapshot::Snapshot,
    output: &str,
    drag: &DragView,
) -> Option<u64> {
    window_source_workspace(drag)
        .filter(|source| {
            compositor
                .workspaces_for_output(output)
                .iter()
                .any(|indicator| indicator.id == *source)
        })
        .or_else(|| compositor.active_workspace_id(output))
}

fn panel_slide_band(slot_context: PanelSlotContext<'_>) -> SlideBand {
    let frame_depth_px = slot_context.config.desktop_panels.height;
    let float_offset_px = slot_context.panel_float_offset_px;
    let window_gap_px = slot_context.compositor_window_gaps_px;
    match slot_context.edge {
        Position::Top => SlideBand::AlongTopEdge {
            frame_depth_px,
            float_offset_px,
            window_gap_px,
        },
        Position::Bottom => SlideBand::AlongBottomEdge {
            frame_depth_px,
            float_offset_px,
            window_gap_px,
        },
        Position::Left => SlideBand::OwnBounds,
    }
}

fn workspace_index(
    compositor: &workspace_window_snapshot::Snapshot,
    output: &str,
    workspace: u64,
) -> u8 {
    compositor
        .workspaces_for_output(output)
        .iter()
        .find(|indicator| indicator.id == workspace)
        .map_or(0, |indicator| indicator.idx)
}

#[derive(Debug)]
pub enum WindowListMessage {
    LauncherEntriesChanged(LauncherEntries),
    JobsChanged(JobProgress),
    ProcessTreeScanned(SnapshotDelivery<Process>),
    WindowProcessesPaired(WindowProcesses),
    ProcessInspected(Box<SnapshotDelivery<Inspection>>),
    PackageFactsRead(Box<SnapshotDelivery<NixPackageFacts>>),
}

#[derive(Debug, Clone)]
pub enum WindowListInteraction {
    ToggleAudio(i32),
    HoverCard(HoverCardMessage),
    ProcessFoldToggled(i32),
    Drag(DragMessage),
}

#[derive(Debug, Clone, Copy)]
pub enum Action {
    None,
    ToggleAudio(i32),
    WindowProcessesPaired,
    Drag(DragMessage),
}

impl WindowList {
    pub fn subscription(agents: &WindowAgentsConfig) -> Subscription<WindowListMessage> {
        Subscription::batch([
            Subscription::run(kde_job_view_progress::job_progress)
                .map(WindowListMessage::JobsChanged),
            if [agents.kitty, agents.wezterm, agents.pty_shells]
                .contains(&TerminalAgentUse::Enabled)
            {
                Subscription::run_with(agents.clone(), |agents| {
                    terminal_emulator_windows::window_pairings(agents.clone())
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
            self.hovered_window = ProcessHover::Idle;
            self.process_tree.stop();
            self.inspection.stop();
            self.package_facts.stop();
            return Task::none();
        };
        self.hovered_window = ProcessHover::Watching(watched.window);
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
                    upstream_branch: config.indicator_groups.window.nixpkgs_branch.clone(),
                    nixos_flake: config.indicator_groups.window.nixos_flake.clone(),
                },
                |fetched| WindowListMessage::PackageFactsRead(Box::new(fetched)),
                config,
                clock,
            ),
        ])
    }

    #[must_use]
    pub fn process_sections(&self) -> Vec<Section> {
        let ProcessHover::Watching(window) = self.hovered_window else {
            return Vec::new();
        };
        [
            self.inspection.sections_described_by(inspection_sections),
            self.package_facts
                .sections_described_by(nix_package_facts_sections),
            self.process_tree.sections_described_by(|tree| {
                let kept = self
                    .window_processes
                    .window_terminal_pids(window)
                    .map_or_else(|| tree.clone(), |kept| tree.pruned_to(kept));
                vec![Section::new(
                    "Processes",
                    process_tree_widget(&kept, &self.folding),
                )]
            }),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    pub fn update(&mut self, msg: WindowListMessage, clock: Clock) -> Action {
        match msg {
            WindowListMessage::LauncherEntriesChanged(entries) => {
                self.launcher = entries;
                Action::None
            }
            WindowListMessage::JobsChanged(jobs) => {
                self.jobs = jobs;
                Action::None
            }
            WindowListMessage::ProcessTreeScanned(fetched) => {
                self.process_tree.receive(fetched, clock);
                Action::None
            }
            WindowListMessage::WindowProcessesPaired(window_processes) => {
                self.window_processes = window_processes;
                Action::WindowProcessesPaired
            }
            WindowListMessage::ProcessInspected(fetched) => {
                self.inspection.receive(*fetched, clock);
                Action::None
            }
            WindowListMessage::PackageFactsRead(fetched) => {
                self.package_facts.receive(*fetched, clock);
                Action::None
            }
        }
    }

    pub fn interact(&mut self, interaction: &WindowListInteraction) -> Action {
        match interaction {
            WindowListInteraction::ToggleAudio(pid) => Action::ToggleAudio(*pid),
            WindowListInteraction::HoverCard(_) => Action::None,
            WindowListInteraction::ProcessFoldToggled(pid) => {
                self.folding.toggle(*pid);
                Action::None
            }
            WindowListInteraction::Drag(drag) => Action::Drag(*drag),
        }
    }

    #[must_use]
    pub fn view<'view>(
        &'view self,
        compositor: &workspace_window_snapshot::Snapshot,
        icons: WindowAssets<'view>,
        audio: Option<&HashMap<i32, WindowAudio>>,
        output: &str,
        slot_context: PanelSlotContext<'_>,
    ) -> Option<Element<'view, WindowListInteraction>> {
        let workspace = shown_workspace(compositor, output, &slot_context.drag)?;
        let arriving = SwapPane {
            identity: workspace,
            slide_order: workspace_index(compositor, output, workspace),
            content: self.workspace_row(compositor, icons, audio, workspace, slot_context),
        };
        let departing =
            match self.departing_workspace(output, workspace, slot_context.clock.instant) {
                DepartingWorkspace::SlidingAway {
                    departed,
                    slide_progress,
                } if window_source_workspace(&slot_context.drag).is_none() => {
                    DepartingPane::Departing {
                        pane: SwapPane {
                            identity: departed.workspace_id,
                            slide_order: departed.workspace_index,
                            content: self.workspace_row(
                                compositor,
                                icons,
                                audio,
                                departed.workspace_id,
                                slot_context,
                            ),
                        },
                        slide_progress,
                    }
                }
                DepartingWorkspace::SlidingAway { .. } | DepartingWorkspace::Absent => {
                    DepartingPane::Absent
                }
            };
        if compositor.windows_on_workspace(workspace).is_empty()
            && matches!(departing, DepartingPane::Absent)
        {
            return None;
        }
        Some(
            sliding_pane_swap(arriving, departing)
                .faded_into(slot_context.panel_frame_style)
                .sliding_within(panel_slide_band(slot_context))
                .into(),
        )
    }

    pub fn observe_active_workspaces(
        &mut self,
        compositor: &workspace_window_snapshot::Snapshot,
        clock: Clock,
        slide_duration: Duration,
    ) {
        compositor
            .active_workspace_ids_by_output()
            .for_each(|(output, workspace_id)| {
                let arrived = ShownWorkspace {
                    workspace_id,
                    workspace_index: workspace_index(compositor, output, workspace_id),
                };
                self.observe_output(output, arrived, clock.instant, slide_duration);
            });
    }

    fn observe_output(
        &mut self,
        output: &str,
        arrived: ShownWorkspace,
        now: Instant,
        slide_duration: Duration,
    ) {
        match self.output_workspaces.get_mut(output) {
            Some(observed) if observed.shown.workspace_id != arrived.workspace_id => {
                observed.workspace_slide = if slide_duration.is_zero() {
                    WorkspaceSlide::Settled
                } else {
                    WorkspaceSlide::Sliding(DepartureSlide {
                        departed: observed.shown,
                        animation: started_slide(now, slide_duration),
                    })
                };
                observed.shown = arrived;
            }
            Some(observed) => observed.shown = arrived,
            None => {
                self.output_workspaces.insert(
                    output.to_owned(),
                    OutputWorkspaces {
                        shown: arrived,
                        workspace_slide: WorkspaceSlide::Settled,
                    },
                );
            }
        }
    }

    #[must_use]
    pub fn workspace_slide_motion(&self, now: Instant) -> MotionState {
        let any_sliding = self.output_workspaces.values().any(|observed| {
            matches!(
                &observed.workspace_slide,
                WorkspaceSlide::Sliding(departure_slide)
                    if departure_slide.animation.is_animating(now)
            )
        });
        if any_sliding {
            MotionState::Moving
        } else {
            MotionState::Settled
        }
    }

    pub fn settle_workspace_slides(&mut self, now: Instant) {
        self.output_workspaces.values_mut().for_each(|observed| {
            if let WorkspaceSlide::Sliding(departure_slide) = &observed.workspace_slide
                && !departure_slide.animation.is_animating(now)
            {
                observed.workspace_slide = WorkspaceSlide::Settled;
            }
        });
    }

    fn departing_workspace(
        &self,
        output: &str,
        arriving_workspace: u64,
        now: Instant,
    ) -> DepartingWorkspace {
        match self.output_workspaces.get(output) {
            Some(OutputWorkspaces {
                workspace_slide: WorkspaceSlide::Sliding(departure_slide),
                ..
            }) if departure_slide.departed.workspace_id != arriving_workspace
                && departure_slide.animation.is_animating(now) =>
            {
                DepartingWorkspace::SlidingAway {
                    departed: departure_slide.departed,
                    slide_progress: departure_slide.animation.interpolate(0.0, 1.0, now),
                }
            }
            Some(_) | None => DepartingWorkspace::Absent,
        }
    }

    fn workspace_row<'view>(
        &'view self,
        compositor: &workspace_window_snapshot::Snapshot,
        icons: WindowAssets<'view>,
        audio: Option<&HashMap<i32, WindowAudio>>,
        workspace: u64,
        slot_context: PanelSlotContext<'_>,
    ) -> Element<'view, WindowListInteraction> {
        let PanelSlotContext {
            config, edge, drag, ..
        } = slot_context;
        let windows = compositor.windows_on_workspace(workspace);
        if windows.is_empty() {
            return Row::new().into();
        }
        let order = drag.previewed(windows, |window_entry| {
            window_slot_key(window_entry, workspace)
        });
        let focused_position = order
            .iter()
            .position(|window_entry| window_entry.focus == WindowFocus::Focused);
        let ArrangedWindowRows {
            groups,
            presentations,
            ungrouped_slot_look,
        } = match slot_context.window_row_layout {
            WindowRowLayout::Flat => arranged_flat_rows(order.len()),
            WindowRowLayout::GroupedByLocation => {
                arranged_keyed_groups(&order, icons, slot_context)
            }
        };
        let window_slots: Vec<_> = order
            .iter()
            .enumerate()
            .map(|(position_in_panel, window_entry)| {
                let item = window_slot_key(window_entry, workspace);
                let opacity = match drag.drag {
                    DragActivity::Dragging(active_drag) if active_drag.item == item => {
                        DISABLED_ALPHA
                    }
                    DragActivity::Dragging(_) | DragActivity::Idle => 1.0,
                };
                WindowSlot {
                    window_entry,
                    item,
                    extras: self.extras(window_entry, audio, config),
                    position_in_panel,
                    opacity,
                    presentation: presentations
                        .get(position_in_panel)
                        .copied()
                        .unwrap_or(WindowSlotPresentation::Flat),
                    slot_context,
                }
            })
            .collect();
        let alarms = window_slots
            .iter()
            .filter(|window_slot| window_slot.urgency_marking() == UrgencyMarking::Tile)
            .map(|window_slot| SlotAlarm {
                position: window_slot.position_in_panel,
                color: Box::new(danger_color),
            })
            .collect();
        let slots = window_slots
            .iter()
            .map(|window_slot| (window_slot.item, window_slot.view(icons)));
        let pager = reorderable_list(slots, edge, config)
            .ungrouped_slot_look(ungrouped_slot_look)
            .alarms(alarms)
            .groups(groups)
            .draggable(PanelSlotList::Windows, &drag, dragged);
        let scrolled_pager = |pager| {
            configured_scroll(pager, Axis::Horizontal, config)
                .edge_fade(ScrollEdgeFade::away_from_frame(
                    slot_context.panel_frame_style,
                    maximum_window_title_width_px(config),
                ))
                .showing(workspace)
        };
        let Some(focused_position) = focused_position else {
            return scrolled_pager(pager).into();
        };
        let focused_key = window_slot_key(order[focused_position], workspace);
        scrolled_pager(pager.highlight(focused_key, primary_color))
            .selected(focused_position)
            .into()
    }

    #[must_use]
    pub fn drag_ghost<'view>(
        &'view self,
        compositor: &workspace_window_snapshot::Snapshot,
        icons: WindowAssets<'view>,
        audio: Option<&HashMap<i32, WindowAudio>>,
        window: u64,
        slot_context: PanelSlotContext<'_>,
    ) -> Option<Element<'view, WindowListInteraction>> {
        let PanelSlotContext { config, drag, .. } = slot_context;
        let workspace = window_source_workspace(&drag)?;
        let window_entry = compositor
            .windows_on_workspace(workspace)
            .iter()
            .find(|window_entry| window_entry.id == window)?;
        let window_slot = WindowSlot {
            window_entry,
            item: window_slot_key(window_entry, workspace),
            extras: self.extras(window_entry, audio, config),
            position_in_panel: 0,
            opacity: 1.0,
            presentation: WindowSlotPresentation::Flat,
            slot_context,
        };
        if drag.list_departure() == ListDeparture::LeftOwnList {
            let inset = config.theme.spacing.surface_inset;
            let circle = drag
                .ghost_frame
                .with_corner_radius(config.theme.font.icon_size_px / 2.0 + inset);
            return Some(
                panel_frame(
                    container(window_slot.slot_icon(icons)?).padding(inset),
                    circle,
                )
                .into(),
            );
        }
        Some(
            panel_frame(
                dragged_slot_content(window_slot.button_content(icons), config),
                drag.ghost_frame,
            )
            .into(),
        )
    }

    #[must_use]
    pub fn window_press_actions(
        &self,
        compositor: &workspace_window_snapshot::Snapshot,
        window_audio: &HashMap<i32, WindowAudio>,
        window: u64,
        config: &Config,
    ) -> Vec<PressAction> {
        let Some(window_entry) = compositor
            .all_windows()
            .find(|window_entry| window_entry.id == window)
        else {
            return Vec::new();
        };
        slot_press_actions(
            window_entry,
            &self.extras(window_entry, Some(window_audio), config),
        )
    }

    fn extras(
        &self,
        window_entry: &workspace_window_snapshot::WinEntry,
        audio: Option<&HashMap<i32, WindowAudio>>,
        config: &Config,
    ) -> Extras {
        let window_config = &config.indicator_groups.window;
        let app_id = match &window_entry.app_id {
            WindowAppId::Reported(app_id) => Some(app_id.as_str()),
            WindowAppId::Unreported => None,
        };
        let launcher = app_id.and_then(|app_id| self.launcher.by_app.get(app_id));
        let job_percent = app_id.and_then(|app_id| self.jobs.get(app_id));
        let audio = match (window_config.audio_indicator, window_entry.pid) {
            (ElementVisibility::Shown, WindowPid::Reported(pid)) => audio
                .and_then(|audio| audio.get(&pid))
                .map_or(WindowSound::Untracked, |window_audio| {
                    WindowSound::Tracked(*window_audio)
                }),
            (ElementVisibility::Shown, WindowPid::Unreported) | (ElementVisibility::Hidden, _) => {
                WindowSound::Untracked
            }
        };
        let progress = if window_config.progress == ElementVisibility::Hidden {
            LauncherProgress::Hidden
        } else if let Some(percent) = job_percent {
            LauncherProgress::Shown(f32::from(percent) / 100.0)
        } else {
            launcher.map_or(LauncherProgress::Hidden, |entry| entry.progress)
        };
        let count = match window_config.count_badge {
            ElementVisibility::Shown => launcher.map_or(LauncherCount::Hidden, |entry| entry.count),
            ElementVisibility::Hidden => LauncherCount::Hidden,
        };
        Extras {
            audio,
            progress,
            count,
            launcher_urgency: launcher.map_or(Urgency::NotUrgent, |entry| entry.urgency),
        }
    }
}

fn window_slot_key(
    window_entry: &workspace_window_snapshot::WinEntry,
    workspace: u64,
) -> DraggedPanelSlot {
    DraggedPanelSlot::Window {
        window: window_entry.id,
        workspace,
    }
}

fn dragged(event: SlotDragEvent) -> WindowListInteraction {
    WindowListInteraction::Drag(DragMessage::Pointer(event))
}
