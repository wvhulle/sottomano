pub mod quick_launch;
pub mod window_list;
pub mod window_tooltip;

use std::{collections::HashMap, sync::Arc};

use iced::{Alignment, Element, Subscription, Task, widget::Row};
use niri_ipc::state::EventStreamState;
use quick_launch::{QuickLaunch, QuickLaunchMessage};
use window_list::{WindowList, WindowListMessage};

use crate::{
    config::{Config, font::FontConfig},
    data::source::{
        app_icon::DesktopEntries, audio::WindowAudio, desktop_entries::DesktopIndex, niri,
    },
    wayland_layers::surface::Position,
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
    ToggleWindowAudio(i32),
    RefreshWindowList,
}

impl WindowGroup {
    pub fn subscription() -> Subscription<WindowGroupMessage> {
        WindowList::subscription().map(WindowGroupMessage::WindowList)
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

    pub fn refresh_window_list(
        &mut self,
        niri: &EventStreamState,
        font: &FontConfig,
    ) -> Task<WindowGroupMessage> {
        self.window_list
            .refresh(niri, font)
            .map(WindowGroupMessage::WindowList)
    }

    pub fn view<'a>(
        &'a self,
        niri: &niri::Snapshot,
        index: &'a DesktopIndex,
        audio: &HashMap<i32, WindowAudio>,
        output: &str,
        config: &Config,
        position: Position,
    ) -> Option<Element<'a, WindowGroupMessage>> {
        let quick_launch = self
            .quick_launch
            .view(index, output, config, position)
            .map(|e| e.map(WindowGroupMessage::QuickLaunch));
        let window_list = self
            .window_list
            .view(niri, index, audio, output, config, position)
            .map(|e| e.map(WindowGroupMessage::WindowList));
        if quick_launch.is_none() && window_list.is_none() {
            return None;
        }
        Some(
            Row::new()
                .extend(quick_launch)
                .extend(window_list)
                .spacing(config.spacing.group_spacing_px)
                .align_y(Alignment::Center)
                .into(),
        )
    }
}
