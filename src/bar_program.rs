//! The bar application: a multi-output daemon with one layer-shell surface per
//! output/edge.

use std::collections::BTreeMap;

use iced::{
    Alignment, Color, Element, Length, Task,
    theme::Style,
    time::Instant,
    widget::{Space, container, mouse_area, stack, text},
    window::Id,
};
use iced_layershell::{actions::LayerShellOutputEvent, to_layer_message};
use niri_ipc::state::{EventStreamState, EventStreamStatePart};

use crate::{
    bar_layout::{
        self,
        battery_group::{BatteryGroup, BatteryMessage},
        clock_group::{ClockGroup, ClockMessage},
        panel_settings_group::{PanelSettingsGroup, PanelSettingsMessage},
        system_group::{SystemGroup, SystemMessage},
        window_list_group::{WindowListGroup, WindowListMessage},
    },
    data_sources::{disk::Disk, niri, sysfs_sensors},
    layer_surface::{Position, Surface, float_margin, layer_settings},
    message_subscriptions::{SourceKind, niri_action, poll},
    tooltip::{self, TooltipHost, TooltipMessage},
    user_config::Config,
};

pub struct Bar {
    pub(crate) config: Config,
    pub(crate) battery_group: BatteryGroup,
    pub(crate) system_group: SystemGroup,
    pub(crate) panel_settings_group: PanelSettingsGroup,
    pub(crate) clock_group: ClockGroup,
    pub(crate) window_list: WindowListGroup,
    pub(crate) niri: EventStreamState,
    pub(crate) surfaces: BTreeMap<Id, Surface>,
    pub(crate) tooltip: TooltipHost,
}

#[to_layer_message(multi)]
#[derive(Debug, Clone)]
pub enum Message {
    Poll(SourceKind),
    Battery(BatteryMessage),
    System(SystemMessage),
    PanelSettings(PanelSettingsMessage),
    Clock(ClockMessage),
    WindowList(WindowListMessage),
    CloseMenus,
    FocusWorkspace(u64),
    Niri(niri_ipc::Event),
    WindowClosed(Id),
    Animate,
    Tooltip(TooltipMessage),
}

impl Bar {
    pub fn new(config: Config) -> (Self, Task<Message>) {
        let mut bar = Self {
            battery_group: BatteryGroup::default(),
            system_group: SystemGroup::new(config.layout.menu_expand_ms),
            panel_settings_group: PanelSettingsGroup::new(config.layout.menu_expand_ms),
            clock_group: ClockGroup::default(),
            window_list: WindowListGroup::default(),
            niri: EventStreamState::default(),
            surfaces: BTreeMap::new(),
            tooltip: TooltipHost::default(),
            config,
        };
        let init = Task::batch([
            bar.window_list.init().map(Message::WindowList),
            Task::batch(SourceKind::ALL.map(|kind| Task::done(Message::Poll(kind)))),
        ]);
        (bar, init)
    }

    pub fn namespace() -> String {
        "odin-rs".to_string()
    }

    /// Whether any expandable group is mid-transition — drives the per-frame
    /// animation subscription. Frame ticking is an app-level concern, so it is
    /// aggregated here across all menu groups rather than exposed per group.
    pub fn is_animating(&self) -> bool {
        self.system_group.menu.is_animating() || self.panel_settings_group.menu.is_animating()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Animate => {
                let now = Instant::now();
                self.system_group.menu.tick(now);
                self.panel_settings_group.menu.tick(now);
                Task::none()
            }
            Message::OutputEvent(LayerShellOutputEvent::Connected { name }) => {
                self.open_bars(&name)
            }
            Message::Poll(kind) => self.poll_source(kind),
            Message::WindowList(WindowListMessage::Clicked(window)) => {
                niri_action(move || niri::focus_window(window))
            }
            Message::WindowList(msg) => self
                .window_list
                .update(msg, &self.niri)
                .map(Message::WindowList),
            Message::Battery(msg) => {
                self.battery_group.update(msg);
                Task::none()
            }
            Message::System(msg) => {
                self.system_group.update(msg);
                Task::none()
            }
            Message::PanelSettings(msg) => self.update_panel_settings(msg),
            Message::Clock(msg) => {
                self.clock_group.update(msg);
                Task::none()
            }
            Message::CloseMenus => {
                self.system_group.update(SystemMessage::Close);
                self.panel_settings_group.update(PanelSettingsMessage::Close);
                Task::none()
            }
            Message::FocusWorkspace(workspace) => {
                niri_action(move || niri::focus_workspace(workspace))
            }
            Message::Niri(event) => self.apply_niri_event(event),
            Message::WindowClosed(id) => {
                self.surfaces.remove(&id);
                self.tooltip.forget(id);
                Task::none()
            }
            Message::Tooltip(msg) => self.tooltip.update(msg, &self.surfaces, &self.config),
            // Disconnected outputs are handled via the WindowClosed each
            // surface receives; the rest are layer-shell actions the runtime
            // intercepts before update.
            Message::OutputEvent(LayerShellOutputEvent::Disconnected { .. })
            | Message::AnchorChange { .. }
            | Message::SetInputRegion { .. }
            | Message::AnchorSizeChange { .. }
            | Message::LayerChange { .. }
            | Message::MarginChange { .. }
            | Message::SizeChange { .. }
            | Message::ExclusiveZoneChange { .. }
            | Message::KeyboardInteractivityChange { .. }
            | Message::VirtualKeyboardPressed { .. }
            | Message::NewLayerShell { .. }
            | Message::NewBaseWindow { .. }
            | Message::NewPopUp { .. }
            | Message::NewMenu { .. }
            | Message::NewInputPanel { .. }
            | Message::RemoveWindow(_)
            | Message::ForgetLastOutput => Task::none(),
        }
    }

    /// Re-resolve favicons for every Firefox window currently open. Firefox's
    /// history/favicon DBs are read synchronously off-thread, keyed by the
    /// brand-stripped page title.
    fn poll_source(&mut self, kind: SourceKind) -> Task<Message> {
        match kind {
            SourceKind::Battery => poll::<sysfs_sensors::battery::Battery>()
                .map(|d| Message::Battery(BatteryMessage::Loaded(d))),
            SourceKind::Cpu => poll::<sysfs_sensors::cpu_usage::CpuUsage>()
                .map(|d| Message::System(SystemMessage::LoadedCpu(d))),
            SourceKind::Memory => poll::<sysfs_sensors::memory::Memory>()
                .map(|d| Message::System(SystemMessage::LoadedMemory(d))),
            SourceKind::Temperature => poll::<sysfs_sensors::temperature::Temperature>()
                .map(|d| Message::System(SystemMessage::LoadedTemperature(d))),
            SourceKind::Disk => {
                poll::<Disk>().map(|d| Message::System(SystemMessage::LoadedDisk(d)))
            }
        }
    }

    fn apply_niri_event(&mut self, event: niri_ipc::Event) -> Task<Message> {
        let window_change = matches!(
            event,
            niri_ipc::Event::WindowsChanged { .. }
                | niri_ipc::Event::WindowOpenedOrChanged { .. }
                | niri_ipc::Event::WindowClosed { .. }
        );
        let _ = self.niri.apply(event);
        if window_change {
            self.window_list
                .refresh(&self.niri)
                .map(Message::WindowList)
        } else {
            Task::none()
        }
    }

    /// The config-mutating knobs are intercepted here rather than handled in
    /// the panel-settings group: the config and the open-surface table (needed
    /// to re-margin open surfaces) both belong to the application. Everything
    /// else is the group's own menu state.
    fn update_panel_settings(&mut self, msg: PanelSettingsMessage) -> Task<Message> {
        match msg {
            PanelSettingsMessage::ToggleFloating => {
                self.config.layout.floating = !self.config.layout.floating;
                self.remargin_surfaces()
            }
            PanelSettingsMessage::SetFloatOffset(offset) => {
                self.config.layout.float_offset_px = offset;
                self.remargin_surfaces()
            }
            msg => {
                self.panel_settings_group.update(msg);
                Task::none()
            }
        }
    }

    /// Re-issue each open surface's margin from the current floating/offset
    /// config — the live counterpart of the margin applied when a surface is
    /// first opened.
    fn remargin_surfaces(&self) -> Task<Message> {
        let offset = if self.config.layout.floating {
            self.config.layout.float_offset_px as i32
        } else {
            0
        };
        Task::batch(self.surfaces.iter().map(|(&id, surface)| {
            Task::done(Message::MarginChange {
                id,
                margin: float_margin(surface.position, offset),
            })
        }))
    }

    fn open_bars(&mut self, output: &str) -> Task<Message> {
        let height = self.config.layout.outer_height_px as u32;
        let offset = if self.config.layout.floating {
            self.config.layout.float_offset_px as i32
        } else {
            0
        };
        let opens: Vec<Task<Message>> = Position::ALL
            .into_iter()
            .map(|position| {
                let id = Id::unique();
                let settings =
                    layer_settings(output, position, height, float_margin(position, offset));
                self.surfaces.insert(
                    id,
                    Surface {
                        output: output.to_string(),
                        position,
                    },
                );
                Task::done(Message::NewLayerShell { settings, id })
            })
            .collect();
        Task::batch(opens)
    }

    pub fn view(&self, id: Id) -> Element<'_, Message> {
        if let Some(tooltip) = self.tooltip.view(id, &self.config) {
            return tooltip;
        }
        let Some(surface) = self.surfaces.get(&id) else {
            return container(text("")).into();
        };
        let content = match surface.position {
            Position::Top => bar_layout::top_row(self, surface.position),
            Position::Bottom => bar_layout::bottom_row(self, &surface.output, surface.position),
        };
        // Stamp this surface's id onto tooltip hovers bubbling out of the bar,
        // so leaf groups need not carry it. Only the bar subtree is wrapped —
        // the tooltip panel returns early above.
        let content = tooltip::on_surface(id, content);

        let dismiss = mouse_area(
            container(Space::new())
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .on_press(Message::CloseMenus);

        container(stack![dismiss, content])
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(Alignment::Center)
            .style(|_| container::Style {
                background: Some(self.config.palette.bar_background().into()),
                border: iced::Border {
                    radius: self.config.chrome_radius().into(),
                    ..Default::default()
                },
                text_color: Some(self.config.palette.bar_foreground()),
                ..Default::default()
            })
            .into()
    }

    pub fn style(&self, _theme: &iced::Theme) -> Style {
        Style {
            background_color: Color::TRANSPARENT,
            text_color: self.config.palette.bar_foreground(),
        }
    }
}
