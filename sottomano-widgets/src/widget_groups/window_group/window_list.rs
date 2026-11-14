use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use corredo::{
    drag_reorder::{HoverReorder, ListDeparture, SlotAlarm, draggable_slot},
    gauges::activity_line,
    orientation::Axis,
    selection_scroll::{ScrollEdgeFade, ScrollPacing, selection_following_scroll},
    theming::{
        background_color, background_strong_color, danger_color, primary_color, warning_color,
    },
    transitions::{DepartingPane, MotionState, SlideBand, SwapPane, sliding_pane_swap},
};
use iced::{
    Alignment, Animation, Element, Subscription, Task,
    animation::Easing,
    widget::{Row, container, progress_bar, right, stack, text, text::Wrapping},
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    desktop_windows_apps::{
        kde_job_view_progress::{self, JobProgress},
        nix_package_facts::{NixPackageFacts, PackageWatch},
        process_tree_inspection::{Inspection, Process, Watched},
        terminal_emulator_windows::{self, WindowProcesses},
        unity_launcher_badges::{self, LauncherEntries, LauncherEntry},
        workspace_window_snapshot,
    },
    media_playback_audio::pipewire_audio::WindowAudio,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::{
    Config,
    screen_edges::Position,
    widget_groups::{ElementVisibility, TerminalAgentUse, WindowAgentsConfig},
};

use crate::{
    clock::Clock,
    desktop_panels::{
        panel_frame::panel_frame,
        panel_slot::{PanelSlot, dragged_slot_content},
        panel_slot_drag::{
            DragMessage, PanelSlotContext, SlotClick, SlotDragEvent, SlotPress,
            window_source_workspace,
        },
    },
    group_items::reading_display_state::{ReadingDisplayState, ReadingStateTint},
    hover_cards::{
        HoverCardMessage, PressHints,
        hover_card_body::{Section, hover_card_source_delivery::SourcedSections},
        hoverable,
    },
    widget_groups::window_group::{
        WindowRowLayout, WindowTarget, WindowTitleLength,
        flat_window_rows::arranged_flat_rows,
        keyed_window_group_rows::{GroupIconPlacement, arranged_keyed_groups},
        process_tree::Folding,
        window_row_presentation::{
            ArrangedWindowRows, SlotBackdrop, WindowSlotPresentation, WindowTileMarking,
        },
    },
    widgets::{
        DISABLED_ALPHA,
        hover_fetched_snapshot::HoverFetchedSnapshot,
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        pointer_drag::{
            DragView, DraggedPanelSlot, PanelSlotList, reorderable_list::reorderable_list,
        },
        program_brand_icons::program_brand_icon,
        shared_metrics::{ACCENT_STROKE_PX, pill_corners},
        simple_icons::view_simple_icon,
        text::font_faces_of,
        window_icon::{WindowAssets, window_icon},
        window_title::WindowTitle,
    },
};

fn maximum_window_title_width_px(config: &Config) -> f32 {
    config
        .widget_groups
        .window
        .resolved_maximum_window_title_width(config.theme.font.text_size_px)
}

fn capped_window_title<'a, Message: 'a>(
    title: impl Into<Element<'a, Message>>,
    config: &Config,
    window_title_length: WindowTitleLength,
) -> Element<'a, Message> {
    match window_title_length {
        WindowTitleLength::Capped => container(title)
            .max_width(maximum_window_title_width_px(config))
            .clip(true)
            .into(),
        WindowTitleLength::Full => title.into(),
    }
}

#[derive(Default)]
pub struct WindowList {
    launcher: LauncherEntries,
    jobs: JobProgress,
    window_processes: WindowProcesses,
    folding: Folding,
    hovered_window: Option<u64>,
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
    match slot_context.edge {
        Position::Top => SlideBand::AlongTopEdge {
            frame_depth_px,
            float_offset_px,
        },
        Position::Bottom => SlideBand::AlongBottomEdge {
            frame_depth_px,
            float_offset_px,
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
                    upstream_branch: config.widget_groups.window.nixpkgs_branch.clone(),
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
                        vec![Section::tree(Some(&kept), &self.folding)]
                    }),
                ]
                .into_iter()
                .flatten()
            })
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

    pub fn interact(&mut self, interaction: WindowListInteraction) -> Action {
        match interaction {
            WindowListInteraction::ToggleAudio(pid) => Action::ToggleAudio(pid),
            WindowListInteraction::HoverCard(_) => Action::None,
            WindowListInteraction::ProcessFoldToggled(pid) => {
                self.folding.toggle(pid);
                Action::None
            }
            WindowListInteraction::Drag(drag) => Action::Drag(drag),
        }
    }

    #[must_use]
    pub fn view<'a>(
        &'a self,
        compositor: &workspace_window_snapshot::Snapshot,
        icons: WindowAssets<'a>,
        audio: Option<&HashMap<i32, WindowAudio>>,
        output: &str,
        slot_context: PanelSlotContext<'_>,
    ) -> Option<Element<'a, WindowListInteraction>> {
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

    fn workspace_row<'a>(
        &'a self,
        compositor: &workspace_window_snapshot::Snapshot,
        icons: WindowAssets<'a>,
        audio: Option<&HashMap<i32, WindowAudio>>,
        workspace: u64,
        slot_context: PanelSlotContext<'_>,
    ) -> Element<'a, WindowListInteraction> {
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
            .position(|window_entry| window_entry.is_focused);
        let ArrangedWindowRows {
            groups,
            presentations,
            ungrouped_highlight_look,
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
                    Some(active_drag) if active_drag.item == item => DISABLED_ALPHA,
                    Some(_) | None => 1.0,
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
        let focused_key = order
            .iter()
            .find(|window_entry| window_entry.is_focused)
            .map(|window_entry| window_slot_key(window_entry, workspace));
        let pager = reorderable_list(slots, edge, config)
            .highlight(focused_key, primary_color)
            .ungrouped_highlight_look(ungrouped_highlight_look)
            .alarms(alarms)
            .groups(groups)
            .draggable(PanelSlotList::Windows, &drag, dragged);
        selection_following_scroll(pager, Axis::Horizontal)
            .edge_fade(ScrollEdgeFade::away_from_frame(
                slot_context.panel_frame_style,
                maximum_window_title_width_px(config),
            ))
            .paced(ScrollPacing::Eased(
                config.desktop_panels.slot_drag.focus_indicator_duration,
            ))
            .selected(focused_position)
            .showing(workspace)
            .into()
    }

    #[must_use]
    pub fn drag_ghost<'a>(
        &'a self,
        compositor: &workspace_window_snapshot::Snapshot,
        icons: WindowAssets<'a>,
        audio: Option<&HashMap<i32, WindowAudio>>,
        window: u64,
        slot_context: PanelSlotContext<'_>,
    ) -> Option<Element<'a, WindowListInteraction>> {
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

    fn extras(
        &self,
        window_entry: &workspace_window_snapshot::WinEntry,
        audio: Option<&HashMap<i32, WindowAudio>>,
        config: &Config,
    ) -> Extras<'_> {
        let app_id = window_entry.app_id.as_deref();
        Extras {
            audio: match config.widget_groups.window.audio_indicator {
                ElementVisibility::Shown => {
                    window_entry.pid.and_then(|pid| audio?.get(&pid)).copied()
                }
                ElementVisibility::Hidden => None,
            },
            launcher: app_id.and_then(|app_id| self.launcher.get(app_id)),
            job: app_id.and_then(|app_id| self.jobs.get(app_id)),
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

#[derive(Clone, Copy, Default)]
struct Extras<'a> {
    audio: Option<WindowAudio>,
    launcher: Option<&'a LauncherEntry>,
    job: Option<u8>,
}

impl Extras<'_> {
    fn progress(&self, config: &Config) -> Option<f32> {
        if config.widget_groups.window.progress == ElementVisibility::Hidden {
            return None;
        }
        self.job
            .map(|percent| f32::from(percent) / 100.0)
            .or_else(|| self.launcher?.progress)
    }

    fn count(&self, config: &Config) -> Option<i64> {
        match config.widget_groups.window.count_badge {
            ElementVisibility::Shown => self.launcher?.count,
            ElementVisibility::Hidden => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UrgencyMarking {
    Tile,
    Line,
    Unmarked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowAttention {
    Urgent,
    Calm,
}

struct WindowSlot<'s> {
    window_entry: &'s workspace_window_snapshot::WinEntry,
    item: DraggedPanelSlot,
    extras: Extras<'s>,
    position_in_panel: usize,
    opacity: f32,
    presentation: WindowSlotPresentation,
    slot_context: PanelSlotContext<'s>,
}

fn glyph_color(
    tint: ReadingStateTint,
    slot_backdrop: SlotBackdrop,
) -> impl Fn(&iced::Theme) -> iced::Color {
    move |theme| match slot_backdrop {
        SlotBackdrop::OpaqueTile => background_color(theme).scale_alpha(tint.opacity),
        SlotBackdrop::Bare => tint.color(theme),
    }
}

impl WindowSlot<'_> {
    fn attention(&self) -> WindowAttention {
        let launcher_urgent = self.extras.launcher.is_some_and(|entry| entry.urgent);
        if self.window_entry.is_urgent || launcher_urgent {
            WindowAttention::Urgent
        } else {
            WindowAttention::Calm
        }
    }

    fn urgency_marking(&self) -> UrgencyMarking {
        match (self.attention(), self.presentation) {
            (WindowAttention::Calm, _) => UrgencyMarking::Unmarked,
            (WindowAttention::Urgent, WindowSlotPresentation::Flat) => UrgencyMarking::Line,
            (
                WindowAttention::Urgent,
                WindowSlotPresentation::InKeyedGroup | WindowSlotPresentation::OutsideKeyedGroups,
            ) => UrgencyMarking::Tile,
        }
    }

    fn tile_marking(&self) -> WindowTileMarking {
        match (self.window_entry.is_focused, self.attention()) {
            (true, _) | (false, WindowAttention::Urgent) => WindowTileMarking::Marked,
            (false, WindowAttention::Calm) => WindowTileMarking::Unmarked,
        }
    }

    fn tile_press(&self) -> TilePress {
        if self.window_entry.is_focused && !self.window_entry.layout.is_floating {
            TilePress::TogglesMaximize
        } else {
            TilePress::FocusesWindow
        }
    }

    fn audio_pid(&self) -> Option<i32> {
        self.window_entry
            .pid
            .filter(|_| self.extras.audio.is_some())
    }

    fn press_hints(&self) -> PressHints {
        PressHints {
            left: Some(if self.tile_press() == TilePress::TogglesMaximize {
                "maximize or restore"
            } else {
                "focus the window"
            }),
            middle: self.audio_pid().map(|_| "mute or unmute its audio"),
            ..PressHints::default()
        }
    }

    fn view<'a>(&self, icons: WindowAssets<'a>) -> Element<'a, WindowListInteraction> {
        let press = SlotPress {
            item: self.item,
            position: self.position_in_panel,
            click: if self.tile_press() == TilePress::TogglesMaximize {
                SlotClick::MaximizeColumn
            } else {
                SlotClick::FocusWindow(self.window_entry.id)
            },
        };
        let hover_reorder =
            if self.window_entry.layout.column.is_some() && !self.window_entry.layout.is_floating {
                HoverReorder::ReordersOnHover
            } else {
                HoverReorder::StaysOnHover
            };
        let pressable = draggable_slot(
            self.panel_slot(icons),
            press,
            hover_reorder,
            &self.slot_context.drag,
            dragged,
        );
        match self.audio_pid() {
            Some(pid) => pressable
                .on_middle_press(WindowListInteraction::ToggleAudio(pid))
                .into(),
            None => pressable.into(),
        }
    }

    fn panel_slot<'a>(&self, icons: WindowAssets<'a>) -> Element<'a, WindowListInteraction> {
        let config = self.slot_context.config;
        let window = self.window_entry.id;
        let press_hints = self.press_hints();
        let usage = self.window_entry.is_focused.then(|| {
            hoverable(
                self.secondary_glyph(Phosphor::UserFocus),
                WindowTarget::Usage(window).into(),
                press_hints,
                WindowListInteraction::HoverCard,
            )
        });
        let inspecting = self.window_entry.is_focused.then(|| {
            hoverable(
                self.secondary_glyph(Phosphor::Package),
                WindowTarget::Inspect(window).into(),
                press_hints,
                WindowListInteraction::HoverCard,
            )
        });
        let content = Row::new()
            .push(self.button_content(icons))
            .extend(usage)
            .extend(inspecting)
            .spacing(config.theme.spacing.item_gap)
            .align_y(Alignment::Center);
        let panel_slot = PanelSlot::new(content.into(), self.indicator_line())
            .padded_along_panel(config.theme.spacing.item_gap);
        match self.progress_ticks() {
            Some(ticks) => panel_slot.ticks(ticks),
            None => panel_slot,
        }
        .view(self.slot_context.edge)
    }

    fn secondary_glyph<'a>(
        &self,
        icon: fn(Weight) -> Phosphor,
    ) -> Element<'a, WindowListInteraction> {
        let tint = ReadingStateTint {
            state: ReadingDisplayState::Off,
            opacity: self.opacity,
        };
        IconGlyph::from(icon(tint.state.weight())).view(
            self.slot_context.config.theme.font.icon_size_px,
            glyph_color(tint, self.slot_backdrop()),
        )
    }

    fn slot_backdrop(&self) -> SlotBackdrop {
        self.presentation.slot_backdrop(self.tile_marking())
    }

    fn button_content<'a>(&self, icons: WindowAssets<'a>) -> Element<'a, WindowListInteraction> {
        let config = self.slot_context.config;
        let title = self.window_entry.label.as_ref().map(|label| {
            self.presentation.styled_title(
                WindowTitle::new(label, config)
                    .opacity(self.opacity)
                    .wrapping(Wrapping::None),
                self.tile_marking(),
            )
        });
        Row::new()
            .extend(self.badged_icon(icons))
            .extend(title.map(|title| {
                capped_window_title(title, config, self.slot_context.window_title_length)
            }))
            .extend(self.audio_glyph())
            .spacing(config.theme.spacing.item_gap)
            .align_y(Alignment::Center)
            .into()
    }

    fn audio_glyph<'a>(&self) -> Option<Element<'a, WindowListInteraction>> {
        let (state, icon) = match self.extras.audio? {
            WindowAudio::Silent => return None,
            WindowAudio::Audible => (
                ReadingDisplayState::On,
                Phosphor::SpeakerHigh(Weight::Regular),
            ),
            WindowAudio::Muted => (
                ReadingDisplayState::Off,
                Phosphor::SpeakerX(Weight::Regular),
            ),
        };
        let tint = ReadingStateTint {
            state,
            opacity: self.opacity,
        };
        Some(IconGlyph::from(icon.with_weight(state.weight())).view(
            self.slot_context.config.theme.font.icon_size_px,
            glyph_color(tint, self.slot_backdrop()),
        ))
    }

    fn indicator_line<'a>(&self) -> Element<'a, WindowListInteraction> {
        let severity = match self.urgency_marking() {
            UrgencyMarking::Line => 1.0,
            UrgencyMarking::Tile | UrgencyMarking::Unmarked => 0.0,
        };
        activity_line(danger_color, severity * self.opacity, ACCENT_STROKE_PX)
    }

    fn progress_ticks<'a>(&self) -> Option<Element<'a, WindowListInteraction>> {
        let config = self.slot_context.config;
        let fraction = self.extras.progress(config)?;
        let thickness = ACCENT_STROKE_PX;
        let opacity = self.opacity;
        Some(
            progress_bar(0.0..=1.0, fraction)
                .girth(thickness)
                .style(move |theme| progress_bar::Style {
                    background: background_strong_color(theme).scale_alpha(opacity).into(),
                    bar: warning_color(theme).scale_alpha(opacity).into(),
                    border: pill_corners(thickness),
                })
                .into(),
        )
    }

    fn count_badge<'a>(&self) -> Option<Element<'a, WindowListInteraction>> {
        let config = self.slot_context.config;
        let count = self.extras.count(config).filter(|count| *count > 0)?;
        let label = if count > 99 {
            "99+".to_owned()
        } else {
            count.to_string()
        };
        let size = config.theme.font.text_size_px;
        let opacity = self.opacity;
        let badge = container(
            text(label)
                .size(size)
                .font(font_faces_of(&config.theme.font).regular)
                .style(move |theme| text::Style {
                    color: Some(background_color(theme).scale_alpha(opacity)),
                }),
        )
        .padding([0.0, config.theme.spacing.item_gap])
        .style(move |theme| {
            container::Style::default()
                .background(danger_color(theme).scale_alpha(opacity))
                .border(pill_corners(size))
        });
        Some(right(badge).into())
    }

    fn badged_icon<'a>(
        &self,
        icons: WindowAssets<'a>,
    ) -> Option<Element<'a, WindowListInteraction>> {
        let badge = self.count_badge();
        if self.group_icon_placement() == Some(GroupIconPlacement::OnHeading) && badge.is_none() {
            return None;
        }
        let icon = self.slot_icon(icons)?;
        Some(match badge {
            Some(badge) => stack([icon, badge]).into(),
            None => icon,
        })
    }

    fn group_icon_placement(&self) -> Option<GroupIconPlacement> {
        match self.presentation {
            WindowSlotPresentation::InKeyedGroup => {
                self.window_entry.group_key().map(GroupIconPlacement::from)
            }
            WindowSlotPresentation::Flat | WindowSlotPresentation::OutsideKeyedGroups => None,
        }
    }

    fn slot_icon<'a>(&self, icons: WindowAssets<'a>) -> Option<Element<'a, WindowListInteraction>> {
        let Some(brand_icon) = self
            .window_entry
            .program_name()
            .and_then(program_brand_icon)
        else {
            return self.resolved_icon(icons);
        };
        let tint = ReadingStateTint {
            state: ReadingDisplayState::Idle,
            opacity: self.opacity,
        };
        Some(view_simple_icon(
            brand_icon,
            self.slot_context.config.theme.font.icon_size_px,
            glyph_color(tint, self.slot_backdrop()),
        ))
    }

    fn resolved_icon<'a>(
        &self,
        icons: WindowAssets<'a>,
    ) -> Option<Element<'a, WindowListInteraction>> {
        window_icon(
            icons.favicons,
            self.window_entry.favicon_key(),
            icons.icon_for(self.window_entry),
            self.slot_context.config.theme.font.icon_size_px,
            self.opacity,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TilePress {
    TogglesMaximize,
    FocusesWindow,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use iced::{
        Element, Settings, Size,
        widget::{text, text::Wrapping},
    };
    use iced_controller_diagram::ControllerArtwork;
    use iced_test::Simulator;
    use sottomano_toml_config::{Config, screen_edges::Position};

    use super::{
        DepartingWorkspace, MotionState, ShownWorkspace, WindowList, WindowListInteraction,
        capped_window_title, maximum_window_title_width_px,
    };
    use crate::{
        clock::Clock,
        desktop_index::DesktopIndex,
        desktop_panels::panel_slot_drag::{
            DragMessage, PanelSlotContext,
            pager_drag_simulation::{
                SIMULATED_OUTPUT, SIMULATED_WINDOW_IDS, SIMULATED_WORKSPACE_IDS,
                dragging_from_start, sweep_across_panel, three_workspaces_three_windows,
                three_workspaces_three_windows_on,
            },
        },
        widget_groups::window_group::{WindowRowLayout, WindowTitleLength},
        widgets::{
            favicon::Favicons,
            pointer_drag::{DragEvent, DraggedPanelSlot, PanelSlotList},
            shell_layer_appearance::ShellLayer,
            window_icon::WindowAssets,
        },
    };

    #[test]
    fn a_departed_workspace_slides_for_the_configured_duration() {
        let slide_duration = Duration::from_millis(250);
        let clock = Clock::real();
        let mut window_list = WindowList::default();
        window_list.observe_active_workspaces(
            &three_workspaces_three_windows(),
            clock,
            slide_duration,
        );
        let arriving_workspace = SIMULATED_WORKSPACE_IDS[1];
        assert_eq!(
            window_list.workspace_slide_motion(clock.instant),
            MotionState::Settled
        );
        window_list.observe_active_workspaces(
            &three_workspaces_three_windows_on(arriving_workspace),
            clock,
            slide_duration,
        );
        assert_eq!(
            window_list.workspace_slide_motion(clock.instant),
            MotionState::Moving
        );
        assert_eq!(
            window_list.departing_workspace(SIMULATED_OUTPUT, arriving_workspace, clock.instant),
            DepartingWorkspace::SlidingAway {
                departed: ShownWorkspace {
                    workspace_id: SIMULATED_WORKSPACE_IDS[0],
                    workspace_index: 1,
                },
                slide_progress: 0.0,
            }
        );
        let after_slide = clock.advanced(slide_duration).instant;
        window_list.settle_workspace_slides(after_slide);
        assert_eq!(
            window_list.workspace_slide_motion(after_slide),
            MotionState::Settled
        );
        assert_eq!(
            window_list.departing_workspace(SIMULATED_OUTPUT, arriving_workspace, after_slide),
            DepartingWorkspace::Absent
        );
    }

    #[test]
    fn a_zero_slide_duration_switches_instantly() {
        let clock = Clock::real();
        let mut window_list = WindowList::default();
        window_list.observe_active_workspaces(
            &three_workspaces_three_windows(),
            clock,
            Duration::ZERO,
        );
        window_list.observe_active_workspaces(
            &three_workspaces_three_windows_on(SIMULATED_WORKSPACE_IDS[1]),
            clock,
            Duration::ZERO,
        );
        assert_eq!(
            window_list.workspace_slide_motion(clock.instant),
            MotionState::Settled
        );
    }

    #[test]
    fn a_very_long_title_is_no_wider_than_the_configured_maximum() {
        let config = Config::default();
        let long_title = "a very long window title ".repeat(12);
        let title_element: Element<'_, ()> =
            text(long_title.clone()).wrapping(Wrapping::None).into();
        let capped = capped_window_title(title_element, &config, WindowTitleLength::Capped);
        let mut simulated_panel =
            Simulator::with_size(Settings::default(), Size::new(1600.0, 40.0), capped);
        let title_target = simulated_panel
            .find(long_title.as_str())
            .expect("the capped title is laid out");
        assert!(title_target.visible_bounds().is_some_and(|bounds| {
            bounds.width <= maximum_window_title_width_px(&config) + f32::EPSILON
        }));
    }

    #[test]
    fn a_full_length_title_grows_past_the_configured_maximum() {
        let config = Config::default();
        let long_title = "a very long window title ".repeat(12);
        let title_element: Element<'_, ()> =
            text(long_title.clone()).wrapping(Wrapping::None).into();
        let uncapped = capped_window_title(title_element, &config, WindowTitleLength::Full);
        let mut simulated_panel =
            Simulator::with_size(Settings::default(), Size::new(4000.0, 40.0), uncapped);
        let title_target = simulated_panel
            .find(long_title.as_str())
            .expect("the full title is laid out");
        assert!(
            title_target
                .visible_bounds()
                .is_some_and(|bounds| bounds.width > maximum_window_title_width_px(&config))
        );
    }

    #[test]
    fn dragging_a_window_across_the_list_enters_each_slot_then_leaves() {
        let config = Config::default();
        let window_snapshot = three_workspaces_three_windows();
        let dragged_window = DraggedPanelSlot::Window {
            window: SIMULATED_WINDOW_IDS[0],
            workspace: SIMULATED_WORKSPACE_IDS[0],
        };
        let drag_view = dragging_from_start(dragged_window);
        let desktop_entries = DesktopIndex::default();
        let favicons = Favicons::default();
        let controller_artwork = ControllerArtwork::decoded();
        let window_assets = WindowAssets {
            entries: &desktop_entries,
            favicons: &favicons,
            controller_artwork: &controller_artwork,
        };
        let window_list = WindowList::default();
        let slot_context = PanelSlotContext {
            config: &config,
            edge: Position::Bottom,
            drag: drag_view,
            clock: Clock::real(),
            panel_corner_radius_px: 0.0,
            panel_float_offset_px: 0.0,
            panel_frame_style: ShellLayer::Panels.default_appearance().frame_style(),
            window_row_layout: WindowRowLayout::default(),
            window_title_length: WindowTitleLength::default(),
        };
        let pager_element = window_list
            .view(
                &window_snapshot,
                window_assets,
                None,
                SIMULATED_OUTPUT,
                slot_context,
            )
            .expect("three windows on the active workspace");
        let drag_events: Vec<DragEvent<_>> =
            sweep_across_panel(pager_element, config.desktop_panels.height)
                .into_iter()
                .filter_map(|window_list_message| match window_list_message {
                    WindowListInteraction::Drag(DragMessage::Pointer(drag_event)) => {
                        Some(drag_event)
                    }
                    _ => None,
                })
                .collect();
        let entered_positions: Vec<usize> = drag_events
            .iter()
            .filter_map(|drag_event| match drag_event {
                DragEvent::EnteredSlot { position, .. } => Some(*position),
                _ => None,
            })
            .collect();
        assert_eq!(entered_positions, [0, 1, 2]);
        assert!(matches!(
            drag_events.last(),
            Some(DragEvent::LeftList(PanelSlotList::Windows))
        ));
    }
}
