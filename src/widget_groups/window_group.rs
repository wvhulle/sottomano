pub mod process_tree;
pub mod quick_launch;
pub mod window_list;
pub mod window_tooltip;

use std::{collections::HashMap, sync::Arc};

use iced::{Alignment, Element, Subscription, Task, widget::Row};
use niri_ipc::state::EventStreamState;
use quick_launch::{QuickLaunch, QuickLaunchMessage};
use window_list::{WindowList, WindowListMessage};

use crate::{
    config::{Config, widget_groups::WindowAgentsConfig},
    data::source::{app_icon::DesktopEntries, audio::WindowAudio, niri, process::Watched},
    desktop_shell::{hover_tooltips::HoverTarget, monitor_surfaces::Position},
    widgets::{tooltip_body::Tooltip, widget_group::GroupEvent, window_icon::WindowAssets},
};

#[derive(Default)]
pub struct WindowGroup {
    pub(crate) quick_launch: QuickLaunch,
    pub(crate) window_list: WindowList,
}

#[derive(Debug, Clone)]
pub enum WindowGroupMessage {
    QuickLaunch(QuickLaunchMessage),
    WindowList(WindowListMessage),
}

pub enum Action {
    None,
    Run(Task<WindowGroupMessage>),
    FocusWindow(u64),
    MaximizeColumn,
    ToggleWindowAudio(i32),
    RefreshWindowList,
}

impl WindowGroup {
    pub fn collapse(&mut self) {
        self.quick_launch.expansion.collapse();
    }

    pub fn subscription(agents: &WindowAgentsConfig) -> Subscription<WindowGroupMessage> {
        WindowList::subscription(agents).map(WindowGroupMessage::WindowList)
    }

    pub fn hover(
        &mut self,
        niri_state: &EventStreamState,
        hovered: Option<&HoverTarget>,
        config: &Config,
    ) -> Task<WindowGroupMessage> {
        let watched = match hovered {
            Some(HoverTarget::Window(window)) => watched(niri_state, *window),
            _ => None,
        };
        self.window_list
            .hover_process(watched, config)
            .map(WindowGroupMessage::WindowList)
    }

    pub fn window_tooltip(&self, niri_state: &EventStreamState, window: u64) -> Option<Tooltip> {
        window_tooltip::tooltip(niri_state, window, &self.window_list.process_sections())
    }

    pub fn update(&mut self, msg: WindowGroupMessage) -> Action {
        match msg {
            WindowGroupMessage::QuickLaunch(m) => Action::Run(
                self.quick_launch
                    .update(m)
                    .map(WindowGroupMessage::QuickLaunch),
            ),
            WindowGroupMessage::WindowList(m) => match self.window_list.update(m) {
                window_list::Action::None => Action::None,
                window_list::Action::FocusWindow(window) => Action::FocusWindow(window),
                window_list::Action::MaximizeColumn => Action::MaximizeColumn,
                window_list::Action::ToggleAudio(pid) => Action::ToggleWindowAudio(pid),
                window_list::Action::RefreshIcons => Action::RefreshWindowList,
            },
        }
    }

    pub fn set_ranked(&mut self, ranked: HashMap<String, Vec<String>>) {
        self.quick_launch.set_ranked(ranked);
    }

    pub fn set_desktop_entries(&mut self, entries: Arc<DesktopEntries>) {
        self.quick_launch.set_desktop_entries(entries);
    }

    pub fn icon_keys(&self) -> Vec<String> {
        self.quick_launch.icon_keys()
    }

    pub fn view<'a>(
        &'a self,
        niri: &niri::Snapshot,
        icons: WindowAssets<'a>,
        audio: &HashMap<i32, WindowAudio>,
        output: &str,
        config: &'a Config,
        position: Position,
    ) -> Option<Element<'a, GroupEvent<WindowGroupMessage>>> {
        let quick_launch = self
            .quick_launch
            .group_content(icons.entries, output, config)
            .map(WindowGroupMessage::QuickLaunch)
            .view(config, position);
        let window_list = self
            .window_list
            .view(niri, icons, audio, output, config, position)
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
                .spacing(config.panel_bar.group_spacing_px)
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
