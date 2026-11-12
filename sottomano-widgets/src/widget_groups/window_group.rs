mod flat_window_rows;
mod keyed_window_group_rows;
pub mod process_tree;
mod window_hover_card;
pub mod window_list;
mod window_row_presentation;

use std::collections::HashMap;

use iced::{Element, Subscription, Task};
use sottomano_desktop_sources::{
    compositor_window_ipc::compositor_window_state::CompositorWindowState,
    desktop_windows_apps::{
        process_tree_inspection::Watched, terminal_emulator_windows::WindowProcesses,
        workspace_window_snapshot,
    },
    media_playback_audio::pipewire_audio::WindowAudio,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::{Config, widget_groups::WindowAgentsConfig};
use window_list::{WindowList, WindowListInteraction, WindowListMessage};

use crate::{
    clock::Clock,
    desktop_panels::panel_slot_drag::PanelSlotContext,
    hover_cards::{
        HoverTarget,
        hover_card_body::hover_card_source_delivery::{DeliveryTiming, HoverCard},
    },
    widget_groups::widget_group_content::GroupEvent,
    widgets::window_icon::WindowAssets,
};

#[derive(Default)]
pub struct WindowGroup {
    pub window_list: WindowList,
}

#[derive(Debug)]
pub enum WindowGroupMessage {
    WindowList(WindowListMessage),
    Interaction(WindowGroupInteraction),
}

#[derive(Debug, Clone)]
pub enum WindowGroupInteraction {
    WindowList(WindowListInteraction),
}

pub enum Action {
    WindowList(window_list::Action),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WindowRowLayout {
    Flat,
    #[default]
    GroupedByLocation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowTarget {
    Pick(u64),
    Usage(u64),
    Inspect(u64),
}

impl From<WindowTarget> for HoverTarget {
    fn from(target: WindowTarget) -> Self {
        HoverTarget::Window(target)
    }
}

impl WindowGroup {
    pub fn subscription(agents: &WindowAgentsConfig) -> Subscription<WindowGroupMessage> {
        WindowList::subscription(agents).map(WindowGroupMessage::WindowList)
    }

    pub fn launcher_entry_subscription(buses: &DesktopBuses) -> Subscription<WindowGroupMessage> {
        WindowList::launcher_entry_subscription(buses).map(WindowGroupMessage::WindowList)
    }

    pub fn hover(
        &mut self,
        compositor: &CompositorWindowState,
        hovered: Option<&HoverTarget>,
        config: &Config,
        clock: Clock,
    ) -> Task<WindowGroupMessage> {
        let watched = match hovered {
            Some(HoverTarget::Window(WindowTarget::Inspect(window))) => {
                watched(compositor, *window)
            }
            _ => None,
        };
        self.window_list
            .hover_process(watched, config, clock)
            .map(WindowGroupMessage::WindowList)
    }

    #[must_use]
    pub fn pick_hover_card(
        compositor: &CompositorWindowState,
        window: u64,
        clock: Clock,
    ) -> Option<HoverCard> {
        Some(HoverCard::sourced(
            window_hover_card::pick_body(compositor, window, clock.wall)?,
            DeliveryTiming::LivePush,
        ))
    }

    #[must_use]
    pub fn inspect_hover_card(&self, clock: Clock) -> HoverCard {
        HoverCard::grouped(self.window_list.process_sections(clock))
    }

    pub fn update(&mut self, msg: WindowGroupMessage, clock: Clock) -> Action {
        match msg {
            WindowGroupMessage::WindowList(window_list_message) => {
                Action::WindowList(self.window_list.update(window_list_message, clock))
            }
            WindowGroupMessage::Interaction(WindowGroupInteraction::WindowList(
                window_list_interaction,
            )) => Action::WindowList(self.window_list.interact(window_list_interaction)),
        }
    }

    #[must_use]
    pub fn window_processes(&self) -> &WindowProcesses {
        self.window_list.window_processes()
    }

    #[must_use]
    pub fn view<'a>(
        &'a self,
        compositor: &workspace_window_snapshot::Snapshot,
        icons: WindowAssets<'a>,
        audio: Option<&HashMap<i32, WindowAudio>>,
        output: &str,
        slot_context: PanelSlotContext<'a>,
    ) -> Option<Element<'a, GroupEvent<WindowGroupInteraction>>> {
        self.window_list
            .view(compositor, icons, audio, output, slot_context)
            .map(|element| {
                element.map(|interaction| match interaction {
                    WindowListInteraction::HoverCard(hover_card) => {
                        GroupEvent::HoverCard(hover_card)
                    }
                    interaction => {
                        GroupEvent::Group(WindowGroupInteraction::WindowList(interaction))
                    }
                })
            })
    }
}

fn watched(compositor: &CompositorWindowState, window: u64) -> Option<Watched> {
    let found = compositor.windows.get(&window)?;
    Some(Watched {
        window,
        pid: found.pid?,
        app_id: found.app_id.clone(),
    })
}
