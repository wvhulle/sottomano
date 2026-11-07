use std::collections::HashMap;

use iced::{
    Alignment, Element, Subscription, Task,
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
    stream_operators::restarting,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::{
    Config,
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
        WindowRowLayout, WindowTarget,
        flat_window_rows::arranged_flat_rows,
        keyed_window_group_rows::arranged_keyed_groups,
        process_tree::Folding,
        window_row_presentation::{
            ArrangedWindowRows, SlotBackdrop, WindowSlotPresentation, WindowTileMarking,
        },
    },
    widgets::{
        DISABLED_ALPHA,
        activity_line::activity_line,
        configured_theme::{
            background_color, background_strong_color, danger_color, primary_color, warning_color,
        },
        edge_alpha_fade::ScrollEdgeFade,
        hover_fetched_snapshot::HoverFetchedSnapshot,
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        pointer_drag::{
            DraggedPanelSlot, ListDeparture, PanelSlotList,
            draggable_slot::{HoverReorder, draggable_slot},
            slot_pager::{SlotAlarm, slot_pager},
        },
        selection_scroller::{ScrollAxis, ScrollPacing, selection_scroller},
        shared_metrics::{ACCENT_STROKE_PX, pill_corners},
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
) -> Element<'a, Message> {
    container(title)
        .max_width(maximum_window_title_width_px(config))
        .clip(true)
        .into()
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
}

#[derive(Debug, Clone)]
pub enum WindowListMessage {
    ToggleAudio(i32),
    LauncherEntriesChanged(LauncherEntries),
    JobsChanged(JobProgress),
    HoverCard(HoverCardMessage),
    ProcessTreeScanned(SnapshotDelivery<Process>),
    WindowProcessesPaired(WindowProcesses),
    ProcessFoldToggled(i32),
    ProcessInspected(Box<SnapshotDelivery<Inspection>>),
    PackageFactsRead(Box<SnapshotDelivery<NixPackageFacts>>),
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
            WindowListMessage::ToggleAudio(pid) => Action::ToggleAudio(pid),
            WindowListMessage::LauncherEntriesChanged(entries) => {
                self.launcher = entries;
                Action::None
            }
            WindowListMessage::JobsChanged(jobs) => {
                self.jobs = jobs;
                Action::None
            }
            WindowListMessage::HoverCard(_) => Action::None,
            WindowListMessage::ProcessTreeScanned(fetched) => {
                self.process_tree.receive(fetched, clock);
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
                self.inspection.receive(*fetched, clock);
                Action::None
            }
            WindowListMessage::PackageFactsRead(fetched) => {
                self.package_facts.receive(*fetched, clock);
                Action::None
            }
            WindowListMessage::Drag(drag) => Action::Drag(drag),
        }
    }

    pub fn view<'a>(
        &'a self,
        compositor: &workspace_window_snapshot::Snapshot,
        icons: WindowAssets<'a>,
        audio: Option<&HashMap<i32, WindowAudio>>,
        output: &str,
        slot_context: PanelSlotContext<'_>,
    ) -> Option<Element<'a, WindowListMessage>> {
        let PanelSlotContext {
            config, edge, drag, ..
        } = slot_context;
        let workspace = window_source_workspace(&drag)
            .filter(|source| {
                compositor
                    .workspaces_for_output(output)
                    .iter()
                    .any(|indicator| indicator.id == *source)
            })
            .or_else(|| compositor.active_workspace_id(output))?;
        let windows = compositor.windows_on_workspace(workspace);
        if windows.is_empty() {
            return None;
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
        let pager = slot_pager(slots, edge, config)
            .highlight(focused_key, primary_color)
            .ungrouped_highlight_look(ungrouped_highlight_look)
            .alarms(alarms)
            .groups(groups)
            .draggable(PanelSlotList::Windows, &drag, dragged);
        Some(
            selection_scroller(pager, ScrollAxis::Horizontal)
                .edge_fade(ScrollEdgeFade::away_from_frame(
                    slot_context.panel_frame_style,
                    maximum_window_title_width_px(config),
                ))
                .paced(ScrollPacing::Eased(
                    config.desktop_panels.slot_drag.focus_indicator_duration,
                ))
                .selected(focused_position)
                .showing(workspace)
                .into(),
        )
    }

    #[must_use]
    pub fn drag_ghost<'a>(
        &'a self,
        compositor: &workspace_window_snapshot::Snapshot,
        icons: WindowAssets<'a>,
        audio: Option<&HashMap<i32, WindowAudio>>,
        window: u64,
        slot_context: PanelSlotContext<'_>,
    ) -> Option<Element<'a, WindowListMessage>> {
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
                    container(window_slot.resolved_icon(icons)?).padding(inset),
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

fn dragged(event: SlotDragEvent) -> WindowListMessage {
    WindowListMessage::Drag(DragMessage::Pointer(event))
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

    fn view<'a>(&self, icons: WindowAssets<'a>) -> Element<'a, WindowListMessage> {
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
                .on_middle_press(WindowListMessage::ToggleAudio(pid))
                .into(),
            None => pressable.into(),
        }
    }

    fn panel_slot<'a>(&self, icons: WindowAssets<'a>) -> Element<'a, WindowListMessage> {
        let config = self.slot_context.config;
        let window = self.window_entry.id;
        let press_hints = self.press_hints();
        let picking = hoverable(
            self.button_content(icons),
            WindowTarget::Pick(window).into(),
            press_hints,
            WindowListMessage::HoverCard,
        );
        let usage = self.window_entry.is_focused.then(|| {
            hoverable(
                self.secondary_glyph(Phosphor::UserFocus),
                WindowTarget::Usage(window).into(),
                press_hints,
                WindowListMessage::HoverCard,
            )
        });
        let inspecting = self.window_entry.is_focused.then(|| {
            hoverable(
                self.secondary_glyph(Phosphor::Package),
                WindowTarget::Inspect(window).into(),
                press_hints,
                WindowListMessage::HoverCard,
            )
        });
        let content = Row::new()
            .push(picking)
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

    fn secondary_glyph<'a>(&self, icon: fn(Weight) -> Phosphor) -> Element<'a, WindowListMessage> {
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

    fn button_content<'a>(&self, icons: WindowAssets<'a>) -> Element<'a, WindowListMessage> {
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
            .extend(title.map(|title| capped_window_title(title, config)))
            .extend(self.audio_glyph())
            .spacing(config.theme.spacing.item_gap)
            .align_y(Alignment::Center)
            .into()
    }

    fn audio_glyph<'a>(&self) -> Option<Element<'a, WindowListMessage>> {
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

    fn indicator_line<'a>(&self) -> Element<'a, WindowListMessage> {
        let severity = match self.urgency_marking() {
            UrgencyMarking::Line => 1.0,
            UrgencyMarking::Tile | UrgencyMarking::Unmarked => 0.0,
        };
        activity_line(danger_color, severity * self.opacity, ACCENT_STROKE_PX)
    }

    fn progress_ticks<'a>(&self) -> Option<Element<'a, WindowListMessage>> {
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

    fn count_badge<'a>(&self) -> Option<Element<'a, WindowListMessage>> {
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

    fn badged_icon<'a>(&self, icons: WindowAssets<'a>) -> Option<Element<'a, WindowListMessage>> {
        let badge = self.count_badge();
        if self.presentation == WindowSlotPresentation::InKeyedGroup && badge.is_none() {
            return None;
        }
        let icon = self.resolved_icon(icons)?;
        Some(match badge {
            Some(badge) => stack([icon, badge]).into(),
            None => icon,
        })
    }

    fn resolved_icon<'a>(&self, icons: WindowAssets<'a>) -> Option<Element<'a, WindowListMessage>> {
        let icon_path = icons.path_for(self.window_entry);
        window_icon(
            icons.favicons,
            self.window_entry.favicon_key(),
            icon_path.as_deref(),
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
    use iced::{
        Element, Settings, Size,
        widget::{text, text::Wrapping},
    };
    use iced_controller_diagram::ControllerArtwork;
    use iced_test::Simulator;
    use sottomano_toml_config::{Config, screen_edges::Position};

    use super::{
        WindowList, WindowListMessage, capped_window_title, maximum_window_title_width_px,
    };
    use crate::{
        clock::Clock,
        desktop_index::DesktopIndex,
        desktop_panels::panel_slot_drag::{
            DragMessage, PanelSlotContext,
            pager_drag_simulation::{
                SIMULATED_OUTPUT, SIMULATED_WINDOW_IDS, SIMULATED_WORKSPACE_IDS,
                dragging_from_start, sweep_across_panel, three_workspaces_three_windows,
            },
        },
        widget_groups::window_group::WindowRowLayout,
        widgets::{
            favicon::Favicons,
            pointer_drag::{DragEvent, DraggedPanelSlot, PanelSlotList},
            shell_layer_appearance::ShellLayer,
            window_icon::WindowAssets,
        },
    };

    #[test]
    fn a_very_long_title_is_no_wider_than_the_configured_maximum() {
        let config = Config::default();
        let long_title = "a very long window title ".repeat(12);
        let title_element: Element<'_, ()> =
            text(long_title.clone()).wrapping(Wrapping::None).into();
        let capped = capped_window_title(title_element, &config);
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
            panel_frame_style: ShellLayer::Panels.default_appearance().frame_style(),
            window_row_layout: WindowRowLayout::default(),
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
                    WindowListMessage::Drag(DragMessage::Pointer(drag_event)) => Some(drag_event),
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
