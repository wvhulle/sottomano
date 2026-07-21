//! The bar application: a multi-output daemon with one layer-shell surface per
//! output/edge.

use std::{collections::BTreeMap, sync::Arc};

use iced::{
    Alignment, Color, Element, Length, Task,
    theme::Style,
    time::Instant,
    widget::{Space, container, mouse_area, stack, text},
    window::Id,
};
use iced_layershell::{actions::LayerShellOutputEvent, to_layer_message};
use niri_ipc::state::{EventStreamState, EventStreamStatePart};
use tokio::task::spawn_blocking;

use crate::{
    bar_layout::{
        self,
        battery_group::{BatteryGroup, BatteryMessage},
        clock_group::{ClockGroup, ClockMessage},
        system_group::{SystemGroup, SystemMessage},
    },
    data_sources::{app_icon::DesktopEntries, disk::Disk, niri, sysfs_sensors},
    layer_surface::{Position, Surface, layer_settings},
    message_subscriptions::{SourceKind, niri_action, poll},
    user_config::{Config, load},
};

pub struct Bar {
    pub(crate) config: Config,
    pub(crate) battery_group: BatteryGroup,
    pub(crate) system_group: SystemGroup,
    pub(crate) clock_group: ClockGroup,
    pub(crate) niri: EventStreamState,
    pub(crate) desktop_entries: Option<Arc<DesktopEntries>>,
    pub(crate) surfaces: BTreeMap<Id, Surface>,
    pub(crate) now: Instant,
}

#[to_layer_message(multi)]
#[derive(Debug, Clone)]
pub enum Message {
    DesktopEntriesLoaded(Arc<DesktopEntries>),
    Poll(SourceKind),
    Battery(BatteryMessage),
    System(SystemMessage),
    Clock(ClockMessage),
    CloseMenus,
    FocusWindow(u64),
    FocusWorkspace(u64),
    Niri(niri_ipc::Event),
    WindowClosed(Id),
    Animate,
}

impl Bar {
    pub fn new() -> (Self, Task<Message>) {
        let config = load();
        (
            Self {
                battery_group: BatteryGroup::default(),
                system_group: SystemGroup::new(config.layout.menu_expand_ms),
                clock_group: ClockGroup::default(),
                niri: EventStreamState::default(),
                desktop_entries: None,
                surfaces: BTreeMap::new(),
                now: Instant::now(),
                config,
            },
            Task::batch([
                Task::perform(
                    async { spawn_blocking(DesktopEntries::load).await },
                    |joined| match joined {
                        Ok(entries) => Message::DesktopEntriesLoaded(Arc::new(entries)),
                        Err(e) => {
                            eprintln!("desktop entries: scan task panicked: {e}");
                            Message::DesktopEntriesLoaded(Arc::new(DesktopEntries::empty()))
                        }
                    },
                ),
                Task::batch(SourceKind::ALL.map(|kind| Task::done(Message::Poll(kind)))),
            ]),
        )
    }

    pub fn namespace() -> String {
        "odin-rs".to_string()
    }

    /// Whether any expandable group is mid-transition — drives the per-frame
    /// animation subscription. Frame ticking is an app-level concern, so it is
    /// aggregated here across all menu groups rather than exposed per group.
    pub fn is_animating(&self, now: Instant) -> bool {
        self.system_group.menu.is_animating(now)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        self.now = Instant::now();
        match message {
            Message::Animate => Task::none(),
            Message::OutputEvent(LayerShellOutputEvent::Connected { name }) => {
                self.open_bars(&name)
            }
            Message::DesktopEntriesLoaded(entries) => {
                self.desktop_entries = Some(entries);
                Task::none()
            }
            Message::Poll(kind) => match kind {
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
            },
            Message::Battery(msg) => self.battery_group.update(msg),
            Message::System(msg) => self.system_group.update(msg, self.now),
            Message::Clock(msg) => self.clock_group.update(msg),
            Message::CloseMenus => self.system_group.update(SystemMessage::Close, self.now),
            Message::FocusWindow(window) => niri_action(move || niri::focus_window(window)),
            Message::FocusWorkspace(workspace) => {
                niri_action(move || niri::focus_workspace(workspace))
            }
            Message::Niri(event) => {
                let _ = self.niri.apply(event);
                Task::none()
            }
            Message::WindowClosed(id) => {
                self.surfaces.remove(&id);
                Task::none()
            }
            _ => Task::none(),
        }
    }

    fn open_bars(&mut self, output: &str) -> Task<Message> {
        let height = self.config.layout.outer_height as u32;
        let opens: Vec<Task<Message>> = Position::ALL
            .into_iter()
            .map(|position| {
                let id = Id::unique();
                let settings = layer_settings(output, position, height);
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
        let Some(surface) = self.surfaces.get(&id) else {
            return container(text("")).into();
        };
        let content = match surface.position {
            Position::Top => bar_layout::top_row(self, surface.position),
            Position::Bottom => bar_layout::bottom_row(self, &surface.output, surface.position),
        };

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
