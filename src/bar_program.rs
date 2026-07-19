//! The `Bar` Elm program: a multi-output daemon (Phase 1) fed by native data
//! sources (Phase 2). One layer-shell surface per (output, edge):
//! `iced_layershell::build_pattern:: daemon` drives many surfaces from one Elm
//! program, and `view(&self, id)` renders whichever surface iced is painting.
//!
//! Shared data (the polled group state, niri state) lives once on `Bar`, each
//! concern owned by the group that renders it
//! (`bar_layout::{battery_group,system_group, clock_group}`) rather than held
//! as raw fields here; only the surface's identity — which output and which
//! edge — is keyed by `window::Id`. This mirrors `../odin/shell.qml`'s
//! `Variants { model: Quickshell.screens }` opening a top and a
//! bottom `BarWindow` per screen.

use std::{collections::BTreeMap, sync::Arc};

use iced::{
    Alignment, Color, Element, Length, Task,
    widget::{Space, container, mouse_area, stack, text},
    window::Id,
};
use iced_layershell::to_layer_message;
use niri_ipc::state::EventStreamState;

use crate::{
    bar_layout,
    bar_layout::{
        battery_group::{BatteryGroup, BatteryMessage},
        clock_group::ClockGroup,
        system_group::{SystemGroup, SystemMessage},
    },
    data_sources::{app_icon::DesktopEntries, disk::Disk, niri, sysfs_sensors},
    layer_surface::{Position, Surface, layer_settings},
    message_subscriptions::{SourceKind, niri_action, poll},
    user_config::Config,
    visual_elements::BAR_FOREGROUND,
};

pub struct Bar {
    pub(crate) config: Config,
    pub(crate) battery_group: BatteryGroup,
    pub(crate) system_group: SystemGroup,
    pub(crate) clock_group: ClockGroup,
    pub(crate) niri: EventStreamState,
    /// Installed `.desktop` entries for window→icon resolution, scanned once at
    /// startup (odin's `DesktopEntries` singleton); `None` until that scan
    /// completes.
    pub(crate) desktop_entries: Option<Arc<DesktopEntries>>,
    /// Every live layer-shell surface, keyed by its iced `window::Id`.
    pub(crate) surfaces: BTreeMap<Id, Surface>,
}

/// `#[to_layer_message(multi)]` injects the multi-surface action variants
/// (`NewLayerShell { settings, id }`, `AnchorSizeChange`, …) this builder
/// needs.
#[to_layer_message(multi)]
#[derive(Debug, Clone)]
pub enum Message {
    OpenBars(Vec<String>),
    DesktopEntriesLoaded(Arc<DesktopEntries>),
    Tick,
    Poll(SourceKind),
    Battery(BatteryMessage),
    System(SystemMessage),
    /// Click the bar background → collapse any open menu.
    CloseMenus,
    /// Click a window-list entry → focus that window.
    FocusWindow(u64),
    /// Click a workspace pill → focus that workspace.
    FocusWorkspace(u64),
    /// Niri event-stream delta.
    Niri(niri_ipc::Event),
    /// Surface gone (e.g. output unplugged).
    WindowClosed(Id),
}

impl Bar {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                config: crate::user_config::load(),
                battery_group: BatteryGroup::default(),
                system_group: SystemGroup::default(),
                clock_group: ClockGroup::default(),
                niri: EventStreamState::default(),
                desktop_entries: None,
                surfaces: BTreeMap::new(),
            },
            Task::batch([
                // Enumerate outputs off the async executor (blocking niri socket), then
                // open the bars. Log-and-empty on failure: no outputs → no bars, rather
                // than a crash.
                Task::perform(
                    async { tokio::task::spawn_blocking(niri::list_outputs).await },
                    |joined| match joined {
                        Ok(Ok(names)) => Message::OpenBars(names),
                        Ok(Err(e)) => {
                            eprintln!("niri: {e}");
                            Message::OpenBars(Vec::new())
                        }
                        Err(e) => {
                            eprintln!("niri: output enumeration task panicked: {e}");
                            Message::OpenBars(Vec::new())
                        }
                    },
                ),
                // Scan installed .desktop entries once (blocking directory walk) for the
                // window list's icon resolution.
                Task::perform(
                    async { tokio::task::spawn_blocking(DesktopEntries::load).await },
                    |joined| match joined {
                        Ok(entries) => Message::DesktopEntriesLoaded(Arc::new(entries)),
                        Err(e) => {
                            eprintln!("desktop entries: scan task panicked: {e}");
                            Message::DesktopEntriesLoaded(Arc::new(DesktopEntries::empty()))
                        }
                    },
                ),
                // Kick an immediate first poll of every source so widgets aren't blank.
                Task::batch(SourceKind::ALL.map(|kind| Task::done(Message::Poll(kind)))),
            ]),
        )
    }

    pub fn namespace() -> String {
        "odin-rs".to_string()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenBars(outputs) => self.open_bars(outputs),
            Message::DesktopEntriesLoaded(entries) => {
                self.desktop_entries = Some(entries);
                Task::none()
            }
            Message::Tick => {
                self.clock_group.tick();
                Task::none()
            }
            Message::Poll(kind) => match kind {
                SourceKind::Battery => {
                    poll::<sysfs_sensors::Battery>().map(|d| Message::Battery(BatteryMessage::Loaded(d)))
                }
                SourceKind::Cpu => poll::<sysfs_sensors::CpuUsage>()
                    .map(|d| Message::System(SystemMessage::LoadedCpu(d))),
                SourceKind::Memory => poll::<sysfs_sensors::Memory>()
                    .map(|d| Message::System(SystemMessage::LoadedMemory(d))),
                SourceKind::Temperature => poll::<sysfs_sensors::Temperature>()
                    .map(|d| Message::System(SystemMessage::LoadedTemperature(d))),
                SourceKind::Disk => poll::<Disk>().map(|d| Message::System(SystemMessage::LoadedDisk(d))),
            },
            Message::Battery(msg) => self.battery_group.update(msg),
            Message::System(msg) => self.system_group.update(msg),
            Message::CloseMenus => self.system_group.update(SystemMessage::Close),
            Message::FocusWindow(window) => niri_action(move || niri::focus_window(window)),
            Message::FocusWorkspace(workspace) => {
                niri_action(move || niri::focus_workspace(workspace))
            }
            Message::Niri(event) => {
                niri::apply(&mut self.niri, event);
                Task::none()
            }
            Message::WindowClosed(id) => {
                self.surfaces.remove(&id);
                Task::none()
            }
            // Layer-shell action variants injected by the macro: no-op here.
            _ => Task::none(),
        }
    }

    /// Open one bar per (output, edge). Each gets a fresh `window::Id`
    /// registered in `surfaces` *before* the surface exists, so the
    /// `view(id)` that follows the compositor's configure already knows
    /// what to draw.
    fn open_bars(&mut self, outputs: Vec<String>) -> Task<Message> {
        let height = self.config.bar_outer_height();
        let opens: Vec<Task<Message>> = outputs
            .iter()
            .flat_map(|output| {
                Position::ALL
                    .into_iter()
                    .map(move |position| (output.clone(), position))
            })
            .map(|(output, position)| {
                let id = Id::unique();
                let settings = layer_settings(&output, position, height);
                self.surfaces.insert(id, Surface { output, position });
                Task::done(Message::NewLayerShell { settings, id })
            })
            .collect();
        Task::batch(opens)
    }

    pub fn view(&self, id: Id) -> Element<'_, Message> {
        let Some(surface) = self.surfaces.get(&id) else {
            // A surface iced knows about but we don't (e.g. an AllScreens hotplug we
            // never registered). Render nothing rather than panic.
            return container(text("")).into();
        };
        let content = match surface.position {
            Position::Top => bar_layout::top_row(self, surface.position),
            Position::Bottom => bar_layout::bottom_row(self, &surface.output, surface.position),
        };

        // Click-anywhere-empty collapses any open menu (odin's dismiss MouseArea). It
        // sits *under* the content, so widget/menu-handle clicks are captured
        // by their own mouse areas and only gap clicks fall through to here.
        let dismiss = mouse_area(
            container(Space::new())
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .on_press(Message::CloseMenus);

        // The bar chrome: `surface` at `bgOpacity`, rounded only when floating —
        // exactly odin's `BarChrome`. The surface itself is fully transparent
        // (see `style`), so at `bgOpacity 0` this paints nothing and any
        // backdrop shows through.
        container(stack![dismiss, content])
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(Alignment::Center)
            .style(|_| container::Style {
                background: Some(self.config.chrome_color().into()),
                border: iced::Border {
                    radius: self.config.chrome_radius().into(),
                    ..Default::default()
                },
                text_color: Some(BAR_FOREGROUND),
                ..Default::default()
            })
            .into()
    }

    pub fn style(&self, _theme: &iced::Theme) -> iced::theme::Style {
        iced::theme::Style {
            // Fully transparent layer surface — the chrome fill (surface at bgOpacity)
            // is painted by the view's container so `bgOpacity 0` yields a genuinely
            // transparent bar, like odin. Requires the compositor to composite alpha.
            background_color: Color::TRANSPARENT,
            text_color: BAR_FOREGROUND,
        }
    }
}
