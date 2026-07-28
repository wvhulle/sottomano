//! The window group: the bottom-centre "windows area" that pairs the
//! [`quick_launch`] rocket with the live [`window_list`], centred as one unit.
//! Its two children are widgets *of* this group — not groups themselves — so
//! their state, messages, and views are owned and composed here; the app talks
//! to the pair through this one group.

pub mod quick_launch;
pub mod window_list;

use std::{collections::HashMap, sync::Arc};

use iced::{Alignment, Element, Subscription, Task, widget::Row};
use niri_ipc::state::EventStreamState;
use quick_launch::{QuickLaunch, QuickLaunchMessage};
use window_list::{WindowList, WindowListMessage};

use crate::{
    config::resolved::{ResolvedConfig, ResolvedFontConfig},
    data_sources::{app_icon::DesktopEntries, niri},
    wayland_layers::surface::Position,
};

pub struct WindowGroup {
    pub(crate) quick_launch: QuickLaunch,
    pub(crate) window_list: WindowList,
}

#[derive(Debug, Clone)]
pub enum WindowGroupMessage {
    QuickLaunch(QuickLaunchMessage),
    WindowList(WindowListMessage),
}

/// What the app must do in response to an update; effects that need app-owned
/// state (niri, the `.desktop` index) are described, not performed.
pub enum Action {
    None,
    Run(Task<WindowGroupMessage>),
    /// Focus this niri window — the app owns the niri connection.
    FocusWindow(u64),
    /// Re-run the window list's resolves — the app owns the inputs and
    /// responds by calling [`WindowGroup::refresh_window_list`].
    RefreshWindowList,
}

impl WindowGroup {
    pub fn new(menu_expand_ms: u64) -> Self {
        Self {
            quick_launch: QuickLaunch::new(menu_expand_ms),
            window_list: WindowList::default(),
        }
    }

    /// Only the window-list widget polls (its icon backstop); quick-launch
    /// re-ranks off niri events and needs no timer.
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
                window_list::Action::RefreshIcons => Action::RefreshWindowList,
            },
        }
    }

    /// Push freshly ranked quick-launch candidates (computed by the app from
    /// its launch tracker) into the quick-launch widget.
    pub fn set_ranked(&mut self, ranked: HashMap<String, Vec<String>>) -> Task<WindowGroupMessage> {
        self.quick_launch
            .set_ranked(ranked)
            .map(WindowGroupMessage::QuickLaunch)
    }

    /// Hand the shared `.desktop` index to the quick-launch widget for its icon
    /// resolution.
    pub fn set_desktop_entries(
        &mut self,
        entries: Arc<DesktopEntries>,
    ) -> Task<WindowGroupMessage> {
        self.quick_launch
            .set_desktop_entries(entries)
            .map(WindowGroupMessage::QuickLaunch)
    }

    /// Re-resolve the window list's icons against a freshly (re)loaded index.
    pub fn on_entries_loaded(
        &mut self,
        niri: &EventStreamState,
        entries: &Arc<DesktopEntries>,
    ) -> Task<WindowGroupMessage> {
        self.window_list
            .on_entries_loaded(niri, entries)
            .map(WindowGroupMessage::WindowList)
    }

    /// Re-run the window list's resolve pipelines against the current window
    /// set.
    pub fn refresh_window_list(
        &mut self,
        niri: &EventStreamState,
        font: &ResolvedFontConfig,
        entries: Option<&Arc<DesktopEntries>>,
    ) -> Task<WindowGroupMessage> {
        self.window_list
            .refresh(niri, font, entries)
            .map(WindowGroupMessage::WindowList)
    }

    /// The launcher and window list, centred as a pair. Each widget hides
    /// itself (`None`) when it has nothing to show, so an empty group is an
    /// empty row.
    pub fn view<'a>(
        &'a self,
        niri: &niri::Snapshot,
        output: &str,
        config: &ResolvedConfig,
        position: Position,
    ) -> Option<Element<'a, WindowGroupMessage>> {
        let quick_launch = self
            .quick_launch
            .view(output, config, position)
            .map(|e| e.map(WindowGroupMessage::QuickLaunch));
        let window_list = self
            .window_list
            .view(niri, output, config, position)
            .map(|e| e.map(WindowGroupMessage::WindowList));
        if quick_launch.is_none() && window_list.is_none() {
            return None;
        }
        Some(
            Row::new()
                .extend(quick_launch)
                .extend(window_list)
                .spacing(config.layout.horizontal_group_spacing_px)
                .align_y(Alignment::Center)
                .into(),
        )
    }
}
