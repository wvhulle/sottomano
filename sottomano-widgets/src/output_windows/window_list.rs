mod slot_extras;
#[cfg(test)]
mod window_list_tests;
mod window_process_hover;
mod window_slot;
mod workspace_row_view;
mod workspace_slide_tracking;

use std::collections::HashMap;

use corredo::transitions::{DepartingPane, SwapPane, sliding_pane_swap};
use iced::{Element, Subscription, widget::container};
use slot_extras::{Extras, WindowSound, slot_press_actions};
use sottomano_compositor_model::compositor_window_facts::{Urgency, WindowAppId, WindowPid};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    desktop_windows_apps::{
        kde_job_view_progress::{self, JobProgress},
        nix_package_facts::NixPackageFacts,
        process_tree_inspection::{Inspection, Process},
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
};
use window_process_hover::ProcessHover;
use workspace_slide_tracking::{
    DepartingWorkspace, OutputWorkspaces, panel_slide_band, workspace_index,
};

use crate::{
    clock::Clock,
    desktop_panels::panel_slot_drag::{DragMessage, PanelSlotContext, window_source_workspace},
    hover_cards::{HoverCardMessage, PressAction},
    icon_catalog::window_icons::WindowAssets,
    output_windows::{WindowTitleLength, process_tree::Folding},
    widgets::{hover_fetched_snapshot::HoverFetchedSnapshot, pointer_drag::DragView},
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
