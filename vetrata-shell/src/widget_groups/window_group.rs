pub mod process_tree;
pub mod quick_launch;
pub mod window_list;
mod window_tooltip;

use std::{collections::HashMap, sync::Arc};

use iced::{Alignment, Element, Subscription, Task, widget::Row};
use niri_ipc::state::EventStreamState;
use quick_launch::{QuickLaunch, QuickLaunchMessage};
use vetrata_config::{Config, screen_edges::Position, widget_groups::WindowAgentsConfig};
use vetrata_sources::{
    app_icon::DesktopEntries, audio::WindowAudio, bus::DesktopBuses,
    kitty_windows::WindowProcesses, niri, process::Watched,
};
use window_list::{WindowList, WindowListMessage};

use crate::{
    desktop_shell::hover_tooltips::HoverTarget,
    widgets::{
        continuous_reading::ContinuousReading, pointer_drag::DragView, tooltip_body::Tooltip,
        widget_group::GroupEvent, window_icon::WindowAssets,
    },
};

#[derive(Default)]
pub struct WindowGroup {
    pub quick_launch: QuickLaunch,
    pub window_list: WindowList,
}

#[derive(Debug, Clone)]
pub enum WindowGroupMessage {
    QuickLaunch(QuickLaunchMessage),
    WindowList(WindowListMessage),
}

pub enum Action {
    Run(Task<WindowGroupMessage>),
    WindowList(window_list::Action),
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
    pub fn collapse(&mut self) {
        self.quick_launch.expansion.collapse();
    }

    pub fn subscription(agents: &WindowAgentsConfig) -> Subscription<WindowGroupMessage> {
        WindowList::subscription(agents).map(WindowGroupMessage::WindowList)
    }

    pub fn launcher_entry_subscription(buses: &DesktopBuses) -> Subscription<WindowGroupMessage> {
        WindowList::launcher_entry_subscription(buses).map(WindowGroupMessage::WindowList)
    }

    pub fn hover(
        &mut self,
        niri_state: &EventStreamState,
        hovered: Option<&HoverTarget>,
        config: &Config,
    ) -> Task<WindowGroupMessage> {
        let watched = match hovered {
            Some(HoverTarget::Window(WindowTarget::Inspect(window))) => {
                watched(niri_state, *window)
            }
            _ => None,
        };
        self.window_list
            .hover_process(watched, config)
            .map(WindowGroupMessage::WindowList)
    }

    #[must_use]
    pub fn pick_tooltip(
        niri_state: &EventStreamState,
        snapshot: &ContinuousReading<niri::Snapshot>,
        window: u64,
    ) -> Option<Tooltip> {
        snapshot.tooltip_with(window_tooltip::pick_body(niri_state, window)?)
    }

    #[must_use]
    pub fn inspect_tooltip(&self) -> Tooltip {
        Tooltip::sections(self.window_list.process_sections())
    }

    pub fn update(&mut self, msg: WindowGroupMessage) -> Action {
        match msg {
            WindowGroupMessage::QuickLaunch(m) => Action::Run(
                self.quick_launch
                    .update(m)
                    .map(WindowGroupMessage::QuickLaunch),
            ),
            WindowGroupMessage::WindowList(m) => Action::WindowList(self.window_list.update(m)),
        }
    }

    #[must_use]
    pub fn window_processes(&self) -> &WindowProcesses {
        self.window_list.window_processes()
    }

    pub fn set_ranked(&mut self, ranked: HashMap<String, Vec<String>>) {
        self.quick_launch.set_ranked(ranked);
    }

    pub fn set_desktop_entries(&mut self, entries: Arc<DesktopEntries>) {
        self.quick_launch.set_desktop_entries(entries);
    }

    #[must_use]
    pub fn icon_keys(&self) -> Vec<String> {
        self.quick_launch.icon_keys()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn view<'a>(
        &'a self,
        niri: &niri::Snapshot,
        icons: WindowAssets<'a>,
        audio: Option<&HashMap<i32, WindowAudio>>,
        output: &str,
        config: &'a Config,
        position: Position,
        drag: &DragView,
    ) -> Option<Element<'a, GroupEvent<WindowGroupMessage>>> {
        let quick_launch = self
            .quick_launch
            .group_content(icons.entries, output, config)
            .map(WindowGroupMessage::QuickLaunch)
            .view(config, position);
        let window_list = self
            .window_list
            .view(niri, icons, audio, output, config, position, drag)
            .map(|element| {
                element.map(|message| match message {
                    WindowListMessage::Tooltip(tooltip) => GroupEvent::Tooltip(tooltip),
                    message => GroupEvent::Group(WindowGroupMessage::WindowList(message)),
                })
            });
        if quick_launch.is_none() && window_list.is_none() {
            return None;
        }
        Some(
            Row::new()
                .extend(quick_launch)
                .extend(window_list)
                .spacing(config.layout.panel.group_spacing)
                .align_y(Alignment::Center)
                .into(),
        )
    }
}

fn watched(niri_state: &EventStreamState, window: u64) -> Option<Watched> {
    let found = niri_state.windows.windows.get(&window)?;
    Some(Watched {
        window,
        pid: found.pid?,
        app_id: found.app_id.clone(),
    })
}
