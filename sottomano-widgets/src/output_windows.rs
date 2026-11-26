mod flat_window_rows;
mod keyed_window_group_rows;
pub mod process_tree;
mod window_hover_card;
pub mod window_list;
mod window_row_presentation;

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use iced::{Element, Subscription, Task};
use sottomano_compositor_model::{
    compositor_window_facts::WindowPid, compositor_window_state::CompositorWindowState,
};
use sottomano_desktop_sources::{
    desktop_windows_apps::{
        process_tree_inspection::Watched, terminal_emulator_windows::WindowProcesses,
        workspace_window_snapshot,
    },
    media_playback_audio::pipewire_audio::WindowAudio,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::{Config, output_windows::WindowAgentsConfig};
use window_list::{WindowList, WindowListInteraction, WindowListMessage};

use crate::{
    clock::Clock,
    desktop_panels::panel_slot_drag::PanelSlotContext,
    group_indicators::indicator_group_content::GroupEvent,
    hover_cards::{HoverTarget, PressAction, hover_card_body::Section},
    icon_catalog::window_icons::WindowAssets,
};

#[derive(Default)]
pub struct OutputWindows {
    pub window_list: WindowList,
}

#[derive(Debug)]
pub enum OutputWindowsMessage {
    WindowList(WindowListMessage),
}

#[derive(Debug, Clone)]
pub enum OutputWindowsInteraction {
    WindowList(WindowListInteraction),
}

#[derive(Debug, Clone, Copy)]
pub enum Action {
    WindowList(window_list::Action),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WindowRowLayout {
    Flat,
    #[default]
    GroupedByLocation,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WindowTitleLength {
    #[default]
    Capped,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowTarget {
    Usage(u64),
    Inspect(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowInspection {
    Inspecting(u64),
    Idle,
}

impl From<Option<&HoverTarget>> for WindowInspection {
    fn from(hovered_target: Option<&HoverTarget>) -> Self {
        match hovered_target {
            Some(HoverTarget::Window(WindowTarget::Inspect(window))) => {
                WindowInspection::Inspecting(*window)
            }
            Some(_) | None => WindowInspection::Idle,
        }
    }
}

impl From<WindowTarget> for HoverTarget {
    fn from(target: WindowTarget) -> Self {
        HoverTarget::Window(target)
    }
}

impl OutputWindows {
    pub fn subscription(agents: &WindowAgentsConfig) -> Subscription<OutputWindowsMessage> {
        WindowList::subscription(agents).map(OutputWindowsMessage::WindowList)
    }

    pub fn launcher_entry_subscription(buses: &DesktopBuses) -> Subscription<OutputWindowsMessage> {
        WindowList::launcher_entry_subscription(buses).map(OutputWindowsMessage::WindowList)
    }

    pub fn follow_window_inspection(
        &mut self,
        compositor: &CompositorWindowState,
        inspection: WindowInspection,
        config: &Config,
        clock: Clock,
    ) -> Task<OutputWindowsMessage> {
        let watched = match inspection {
            WindowInspection::Inspecting(window) => watched(compositor, window),
            WindowInspection::Idle => None,
        };
        self.window_list
            .hover_process(watched, config, clock)
            .map(OutputWindowsMessage::WindowList)
    }

    #[must_use]
    pub fn window_press_actions(
        &self,
        compositor: &workspace_window_snapshot::Snapshot,
        window_audio: &HashMap<i32, WindowAudio>,
        window: u64,
        config: &Config,
    ) -> Vec<PressAction> {
        self.window_list
            .window_press_actions(compositor, window_audio, window, config)
    }

    #[must_use]
    pub fn window_identity_hover_card(
        compositor: &CompositorWindowState,
        window: u64,
        clock: Clock,
    ) -> Vec<Section> {
        window_hover_card::window_identity_body(compositor, window, clock.wall).unwrap_or_default()
    }

    pub fn update(&mut self, msg: OutputWindowsMessage, clock: Clock) -> Action {
        let OutputWindowsMessage::WindowList(window_list_message) = msg;
        Action::WindowList(self.window_list.update(window_list_message, clock))
    }

    pub fn interact(&mut self, interaction: OutputWindowsInteraction) -> Action {
        let OutputWindowsInteraction::WindowList(window_list_interaction) = interaction;
        Action::WindowList(self.window_list.interact(&window_list_interaction))
    }

    #[must_use]
    pub fn window_processes(&self) -> &WindowProcesses {
        &self.window_list.window_processes
    }

    pub fn observe_active_workspaces(
        &mut self,
        compositor: &workspace_window_snapshot::Snapshot,
        clock: Clock,
        slide_duration: Duration,
    ) {
        self.window_list
            .observe_active_workspaces(compositor, clock, slide_duration);
    }

    pub fn settle_workspace_slides(&mut self, now: Instant) {
        self.window_list.settle_workspace_slides(now);
    }

    #[must_use]
    pub fn view<'view>(
        &'view self,
        compositor: &workspace_window_snapshot::Snapshot,
        icons: WindowAssets<'view>,
        audio: Option<&HashMap<i32, WindowAudio>>,
        output: &str,
        slot_context: PanelSlotContext<'view>,
    ) -> Option<Element<'view, GroupEvent>> {
        self.window_list
            .view(compositor, icons, audio, output, slot_context)
            .map(|element| {
                element.map(|interaction| match interaction {
                    WindowListInteraction::HoverCard(hover_card) => GroupEvent::from(hover_card),
                    interaction => {
                        GroupEvent::from(OutputWindowsInteraction::WindowList(interaction))
                    }
                })
            })
    }
}

fn watched(compositor: &CompositorWindowState, window: u64) -> Option<Watched> {
    let found = compositor.windows.get(&window)?;
    let WindowPid::Reported(pid) = found.pid else {
        return None;
    };
    Some(Watched {
        window,
        pid,
        app_id: found.app_id.clone(),
    })
}
