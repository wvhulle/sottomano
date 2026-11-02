use std::{collections::HashMap, ops::Range, path::PathBuf};

use iced::{
    Alignment, Element, Subscription, Task,
    widget::{Row, container, progress_bar, right, stack, text},
};
use sottomano_config::{Config, widget_groups::WindowAgentsConfig};
use sottomano_sources::{
    SnapshotDelivery,
    desktop_windows_apps::{
        kde_job_view_progress::{self, JobProgress},
        kitty_terminal_windows::{self, WindowProcesses},
        niri_window_snapshot,
        nix_package_facts::{NixPackageFacts, PackageWatch},
        process_tree_inspection::{Inspection, Process, Watched},
        unity_launcher_badges::{self, LauncherEntries, LauncherEntry},
        window_title_labels::WindowLabel,
    },
    media_playback_audio::pipewire_audio::WindowAudio,
    stream_operators::restarting,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    clock::Clock,
    desktop_panels::{
        panel_frame::panel_frame,
        panel_slot::{PanelSlot, dragged_slot_content},
        panel_slot_drag::{DragMessage, PanelSlotContext, SlotClick, SlotDragEvent, SlotPress},
    },
    group_items::reading_display_state::{ReadingDisplayState, ReadingStateTint},
    hover_cards::{
        HoverCardMessage, PressHints,
        hover_card_body::{Section, hover_card_source_delivery::SourcedSections},
        hoverable,
    },
    widget_groups::window_group::{
        WindowTarget,
        process_tree::Folding,
        window_list_viewport::{Metrics, window_list_viewport},
    },
    widgets::{
        DISABLED_ALPHA,
        activity_line::activity_line,
        configured_theme::{
            background_color, background_strong_color, background_weak_color, danger_color,
            primary_color, warning_color,
        },
        hover_fetched_snapshot::HoverFetchedSnapshot,
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        pointer_drag::{
            DraggedPanelSlot, PanelSlotList, draggable_slot::draggable_slot, slot_group::SlotGroup,
            slot_pager::slot_pager,
        },
        shared_metrics::{ACCENT_STROKE_PX, pill_corners},
        text::{FontFaces, LINE_HEIGHT},
        window_icon::{WindowAssets, window_icon},
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

    pub fn view<'a>(
        &'a self,
        niri: &niri_window_snapshot::Snapshot,
        icons: WindowAssets<'a>,
        audio: Option<&HashMap<i32, WindowAudio>>,
        output: &str,
        slot_context: PanelSlotContext<'_>,
    ) -> Option<Element<'a, WindowListMessage>> {
        let PanelSlotContext {
            config, edge, drag, ..
        } = slot_context;
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
        let order = drag.previewed(windows, |window_entry| {
            window_slot_key(window_entry, workspace)
        });
        let focused_position = order
            .iter()
            .position(|window_entry| window_entry.is_focused);
        let group_identities: Vec<_> = order
            .iter()
            .map(|window_entry| WindowGroupIdentity {
                label: window_entry.label.as_ref(),
                icon_path: icons.path_for(window_entry),
                favicon_key: window_entry.favicon_key(),
            })
            .collect();
        let group_positions = location_group_positions(&group_identities);
        let location_groups =
            self.location_slot_groups(&order, &group_positions, icons, slot_context);
        let slots = order
            .iter()
            .enumerate()
            .map(|(position_in_panel, window_entry)| {
                let item = window_slot_key(window_entry, workspace);
                let dimming = if drag.is_dragging(item) {
                    DISABLED_ALPHA
                } else {
                    1.0
                };
                let in_location_group = group_positions
                    .iter()
                    .any(|positions| positions.contains(&position_in_panel));
                let window_slot = WindowSlot {
                    window_entry,
                    item,
                    extras: self.extras(window_entry, audio, config),
                    position_in_panel,
                    opacity: self.metrics.item_opacity(self.scroll, position_in_panel) * dimming,
                    in_location_group,
                    slot_context,
                };
                (item, window_slot.view(icons))
            });
        let focus_opacity = focused_position.map_or(1.0, |focused_position| {
            self.metrics.item_opacity(self.scroll, focused_position)
        });
        let focused_key = order
            .iter()
            .find(|window_entry| window_entry.is_focused)
            .map(|window_entry| window_slot_key(window_entry, workspace));
        let pager = slot_pager(slots, edge, config)
            .highlight(focused_key, move |theme| {
                primary_color(theme).scale_alpha(focus_opacity)
            })
            .groups(location_groups)
            .draggable(PanelSlotList::Windows, &drag, dragged);
        Some(window_list_viewport(
            pager,
            self.scroll,
            focused_position,
            WindowListMessage::Scrolled,
            WindowListMessage::MetricsChanged,
        ))
    }

    fn location_slot_groups<'a>(
        &self,
        order: &[&niri_window_snapshot::WinEntry],
        group_positions: &[Range<usize>],
        icons: WindowAssets<'a>,
        slot_context: PanelSlotContext<'_>,
    ) -> Vec<SlotGroup<'a, WindowListMessage>> {
        group_positions
            .iter()
            .filter_map(|positions| {
                let opacity = positions
                    .clone()
                    .map(|position| self.metrics.item_opacity(self.scroll, position))
                    .fold(0.0, f32::max);
                Some(SlotGroup {
                    positions: positions.clone(),
                    heading: location_group_heading(
                        order.get(positions.start)?,
                        icons,
                        slot_context.config,
                        opacity,
                    )?,
                    enclosure_color: Box::new(move |theme| {
                        background_weak_color(theme).scale_alpha(opacity)
                    }),
                    inner_color: Box::new(background_color),
                    highlight_color: Box::new(move |theme| {
                        primary_color(theme).scale_alpha(opacity)
                    }),
                    enclosure_thickness: slot_context.config.theme.spacing.item_gap,
                    enclosure_radius: slot_context.panel_corner_radius_px,
                })
            })
            .collect()
    }

    #[must_use]
    pub fn drag_ghost<'a>(
        &'a self,
        niri: &niri_window_snapshot::Snapshot,
        icons: WindowAssets<'a>,
        audio: Option<&HashMap<i32, WindowAudio>>,
        window: u64,
        slot_context: PanelSlotContext<'_>,
    ) -> Option<Element<'a, WindowListMessage>> {
        let PanelSlotContext { config, drag, .. } = slot_context;
        let workspace = drag.window_source_workspace()?;
        let window_entry = niri
            .windows_on_workspace(workspace)
            .iter()
            .find(|window_entry| window_entry.id == window)?;
        let window_slot = WindowSlot {
            window_entry,
            item: window_slot_key(window_entry, workspace),
            extras: self.extras(window_entry, audio, config),
            position_in_panel: 0,
            opacity: 1.0,
            in_location_group: false,
            slot_context,
        };
        if drag.has_left_its_list() {
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
        window_entry: &niri_window_snapshot::WinEntry,
        audio: Option<&HashMap<i32, WindowAudio>>,
        config: &Config,
    ) -> Extras<'_> {
        let app_id = window_entry.app_id.as_deref();
        Extras {
            audio: config
                .widget_groups
                .window
                .audio_indicator
                .then(|| window_entry.pid.and_then(|pid| audio?.get(&pid)).copied())
                .flatten(),
            launcher: app_id.and_then(|app_id| self.launcher.get(app_id)),
            job: app_id.and_then(|app_id| self.jobs.get(app_id)),
        }
    }
}

fn window_slot_key(
    window_entry: &niri_window_snapshot::WinEntry,
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
        if !config.widget_groups.window.progress {
            return None;
        }
        self.job
            .map(|percent| f32::from(percent) / 100.0)
            .or_else(|| self.launcher?.progress)
    }

    fn count(&self, config: &Config) -> Option<i64> {
        config
            .widget_groups
            .window
            .count_badge
            .then(|| self.launcher?.count)
            .flatten()
    }

    fn is_urgent(&self) -> bool {
        self.launcher.is_some_and(|entry| entry.urgent)
    }
}

struct WindowSlot<'s> {
    window_entry: &'s niri_window_snapshot::WinEntry,
    item: DraggedPanelSlot,
    extras: Extras<'s>,
    position_in_panel: usize,
    opacity: f32,
    in_location_group: bool,
    slot_context: PanelSlotContext<'s>,
}

struct WindowGroupIdentity<'a> {
    label: Option<&'a WindowLabel>,
    icon_path: Option<PathBuf>,
    favicon_key: Option<&'a str>,
}

impl WindowGroupIdentity<'_> {
    fn continues_group_of(&self, previous: &WindowGroupIdentity<'_>) -> bool {
        self.icon_path == previous.icon_path
            && self.favicon_key == previous.favicon_key
            && self
                .label
                .zip(previous.label)
                .is_some_and(|(label, previous_label)| label.continues_location_of(previous_label))
    }

    fn opens_group(&self) -> bool {
        self.label
            .is_some_and(|label| label.location_text().is_some())
    }
}

fn location_group_positions(identities: &[WindowGroupIdentity<'_>]) -> Vec<Range<usize>> {
    identities
        .chunk_by(|previous, current| current.continues_group_of(previous))
        .scan(0, |next_start, run| {
            let start = *next_start;
            *next_start += run.len();
            Some(start..*next_start)
        })
        .filter(|positions| identities[positions.start].opens_group())
        .collect()
}

fn glyph_color(
    tint: ReadingStateTint,
    on_accent_tile: bool,
) -> impl Fn(&iced::Theme) -> iced::Color {
    move |theme| {
        if on_accent_tile {
            background_color(theme).scale_alpha(tint.opacity)
        } else {
            tint.color(theme)
        }
    }
}

fn location_group_heading<'a>(
    window_entry: &niri_window_snapshot::WinEntry,
    icons: WindowAssets<'a>,
    config: &Config,
    opacity: f32,
) -> Option<Element<'a, WindowListMessage>> {
    let location = window_entry.label.as_ref()?.location_text()?;
    let tint = ReadingStateTint {
        state: ReadingDisplayState::Idle,
        opacity,
    };
    let icon_path = icons.path_for(window_entry);
    let icon = window_icon(
        icons.favicons,
        window_entry.favicon_key(),
        icon_path.as_deref(),
        config.theme.font.icon_size_px,
        opacity,
    );
    Some(
        Row::new()
            .extend(icon)
            .push(
                text(location.to_owned())
                    .size(config.theme.font.text_size_px)
                    .line_height(LINE_HEIGHT)
                    .font(FontFaces::from(&config.theme.font).regular)
                    .style(tint.text_style()),
            )
            .spacing(config.theme.spacing.item_gap)
            .padding([0.0, config.theme.spacing.item_gap])
            .align_y(Alignment::Center)
            .into(),
    )
}

impl WindowSlot<'_> {
    fn maximize_toggles(&self) -> bool {
        self.window_entry.is_focused && !self.window_entry.layout.is_floating
    }

    fn audio_pid(&self) -> Option<i32> {
        self.window_entry
            .pid
            .filter(|_| self.extras.audio.is_some())
    }

    fn press_hints(&self) -> PressHints {
        PressHints {
            left: Some(if self.maximize_toggles() {
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
            click: if self.maximize_toggles() {
                SlotClick::MaximizeColumn
            } else {
                SlotClick::FocusWindow(self.window_entry.id)
            },
        };
        let tiled =
            self.window_entry.layout.column.is_some() && !self.window_entry.layout.is_floating;
        let pressable = draggable_slot(
            self.panel_slot(icons),
            press,
            tiled,
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
            glyph_color(tint, self.sits_on_accent_tile()),
        )
    }

    fn sits_on_accent_tile(&self) -> bool {
        self.in_location_group && self.window_entry.is_focused
    }

    fn button_content<'a>(&self, icons: WindowAssets<'a>) -> Element<'a, WindowListMessage> {
        let config = self.slot_context.config;
        let title = self.window_entry.label.as_ref().map(|label| {
            let window_title = WindowTitle::new(label, config).opacity(self.opacity);
            match (self.in_location_group, self.sits_on_accent_tile()) {
                (false, _) => window_title,
                (true, false) => window_title.without_location(),
                (true, true) => window_title.without_location().color(background_color),
            }
        });
        Row::new()
            .extend(self.badged_icon(icons))
            .extend(title.map(Element::from))
            .extend(self.audio_glyph())
            .spacing(config.theme.spacing.item_gap)
            .align_y(Alignment::Center)
            .into()
    }

    fn audio_glyph<'a>(&self) -> Option<Element<'a, WindowListMessage>> {
        let audio = self.extras.audio?;
        if !audio.playing && !audio.muted {
            return None;
        }
        let state = ReadingDisplayState::of_toggle(!audio.muted);
        let icon = if audio.muted {
            Phosphor::SpeakerX(Weight::Regular)
        } else {
            Phosphor::SpeakerHigh(Weight::Regular)
        };
        let tint = ReadingStateTint {
            state,
            opacity: self.opacity,
        };
        Some(IconGlyph::from(icon.with_weight(state.weight())).view(
            self.slot_context.config.theme.font.icon_size_px,
            glyph_color(tint, self.sits_on_accent_tile()),
        ))
    }

    fn indicator_line<'a>(&self) -> Element<'a, WindowListMessage> {
        let severity = if self.window_entry.is_urgent || self.extras.is_urgent() {
            1.0
        } else {
            0.0
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
                .font(FontFaces::from(&config.theme.font).regular)
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
        if self.in_location_group && badge.is_none() {
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

#[cfg(test)]
mod tests {
    use iced_controller_diagram::ControllerArtwork;
    use sottomano_config::{Config, screen_edges::Position};
    use sottomano_sources::desktop_windows_apps::{
        kitty_terminal_windows::kitty_window_label::{
            ForegroundActivity, KittyWindowLabel, WorkingLocation,
        },
        window_title_labels::WindowLabel,
    };

    use super::{WindowGroupIdentity, WindowList, WindowListMessage, location_group_positions};
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
        widgets::{
            favicon::Favicons,
            pointer_drag::{DragEvent, DraggedPanelSlot, PanelSlotList},
            window_icon::WindowAssets,
        },
    };

    fn kitty_label_in(directory: &str, foreground_activity: ForegroundActivity) -> WindowLabel {
        WindowLabel::KittyAgent(KittyWindowLabel {
            working_location: WorkingLocation::LocalDirectory(directory.to_owned()),
            foreground_activity,
        })
    }

    fn running(command_line: &str) -> ForegroundActivity {
        ForegroundActivity::ShellCommand {
            command_line: command_line.to_owned(),
            program_name: None,
        }
    }

    fn identity_with_icon<'a>(label: &'a WindowLabel, icon: &str) -> WindowGroupIdentity<'a> {
        WindowGroupIdentity {
            label: Some(label),
            icon_path: Some(icon.into()),
            favicon_key: None,
        }
    }

    #[test]
    fn neighbouring_windows_with_one_location_and_icon_form_one_group() {
        let first_session = kitty_label_in("sottomano", running("first session"));
        let second_session = kitty_label_in("sottomano", running("second session"));
        let cargo = kitty_label_in("sottomano", running("cargo"));
        let idle_shell = kitty_label_in(
            "sottomano",
            ForegroundActivity::ShellPrompt {
                shell_name: "nu".to_owned(),
            },
        );
        let plain_title = WindowLabel::Titled("Files".to_owned());
        let other_directory = kitty_label_in("nix", running("jj"));
        let identities = [
            identity_with_icon(&first_session, "claude"),
            identity_with_icon(&second_session, "claude"),
            identity_with_icon(&cargo, "terminal"),
            identity_with_icon(&idle_shell, "terminal"),
            identity_with_icon(&plain_title, "files"),
            identity_with_icon(&other_directory, "terminal"),
            WindowGroupIdentity {
                label: None,
                icon_path: None,
                favicon_key: None,
            },
        ];
        assert_eq!(location_group_positions(&identities), [0..2, 2..4, 5..6]);
    }

    #[test]
    fn dragging_a_window_across_the_list_enters_each_slot_then_leaves() {
        let config = Config::default();
        let niri_snapshot = three_workspaces_three_windows();
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
        };
        let pager_element = window_list
            .view(
                &niri_snapshot,
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
