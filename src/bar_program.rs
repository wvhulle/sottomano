//! The bar application: a multi-output daemon with one layer-shell surface per
//! output/edge.

use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::Arc,
};

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
        panel_settings_group::{PanelSettingsGroup, PanelSettingsMessage},
        system_group::{SystemGroup, SystemMessage},
    },
    data_sources::{app_icon, app_icon::DesktopEntries, disk::Disk, favicons, niri, sysfs_sensors},
    layer_surface::{Position, Surface, float_margin, layer_settings},
    message_subscriptions::{SourceKind, niri_action, poll},
    tooltip::{self, TooltipHost, TooltipMessage},
    user_config::{Config, load},
};

pub struct Bar {
    pub(crate) config: Config,
    pub(crate) battery_group: BatteryGroup,
    pub(crate) system_group: SystemGroup,
    pub(crate) panel_settings_group: PanelSettingsGroup,
    pub(crate) clock_group: ClockGroup,
    pub(crate) niri: EventStreamState,
    pub(crate) desktop_entries: Option<Arc<DesktopEntries>>,
    pub(crate) favicons: HashMap<String, PathBuf>,
    pub(crate) app_icons: HashMap<String, PathBuf>,
    pub(crate) theme_icons: HashMap<String, PathBuf>,
    pub(crate) surfaces: BTreeMap<Id, Surface>,
    pub(crate) now: Instant,
    pub(crate) tooltip: TooltipHost,
}

#[to_layer_message(multi)]
#[derive(Debug, Clone)]
pub enum Message {
    DesktopEntriesLoaded(Arc<DesktopEntries>),
    FaviconsLoaded(HashMap<String, PathBuf>),
    AppIconsResolved(HashMap<String, PathBuf>),
    ThemeIconsResolved(HashMap<String, PathBuf>),
    Poll(SourceKind),
    Battery(BatteryMessage),
    System(SystemMessage),
    PanelSettings(PanelSettingsMessage),
    Clock(ClockMessage),
    CloseMenus,
    FocusWindow(u64),
    FocusWorkspace(u64),
    Niri(niri_ipc::Event),
    WindowClosed(Id),
    Animate,
    Tooltip(TooltipMessage),
}

impl Bar {
    pub fn new() -> (Self, Task<Message>) {
        let config = load();
        (
            Self {
                battery_group: BatteryGroup::default(),
                system_group: SystemGroup::new(config.layout.menu_expand_ms),
                panel_settings_group: PanelSettingsGroup::new(config.layout.menu_expand_ms),
                clock_group: ClockGroup::default(),
                niri: EventStreamState::default(),
                desktop_entries: None,
                favicons: HashMap::new(),
                app_icons: HashMap::new(),
                theme_icons: HashMap::new(),
                surfaces: BTreeMap::new(),
                now: Instant::now(),
                tooltip: TooltipHost::default(),
                config,
            },
            Task::batch([
                Self::load_desktop_entries(),
                Task::batch(SourceKind::ALL.map(|kind| Task::done(Message::Poll(kind)))),
            ]),
        )
    }

    /// Scan `.desktop` entries + the configured icon theme off-thread, so a
    /// newly installed app or a live theme switch is picked up without a
    /// restart.
    fn load_desktop_entries() -> Task<Message> {
        Task::perform(
            async { spawn_blocking(DesktopEntries::load).await },
            |joined| match joined {
                Ok(entries) => Message::DesktopEntriesLoaded(Arc::new(entries)),
                Err(e) => {
                    eprintln!("desktop entries: scan task panicked: {e}");
                    Message::DesktopEntriesLoaded(Arc::new(DesktopEntries::empty()))
                }
            },
        )
    }

    pub fn namespace() -> String {
        "odin-rs".to_string()
    }

    /// Whether any expandable group is mid-transition — drives the per-frame
    /// animation subscription. Frame ticking is an app-level concern, so it is
    /// aggregated here across all menu groups rather than exposed per group.
    pub fn is_animating(&self, now: Instant) -> bool {
        self.system_group.menu.is_animating(now) || self.panel_settings_group.menu.is_animating(now)
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
                self.app_icons.clear();
                self.theme_icons.clear();
                Task::batch([self.poll_app_icons(), self.poll_theme_icons()])
            }
            Message::FaviconsLoaded(favicons) => {
                self.favicons = favicons;
                Task::none()
            }
            Message::AppIconsResolved(map) => {
                self.app_icons = map;
                Task::none()
            }
            Message::ThemeIconsResolved(map) => {
                self.theme_icons = map;
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
                SourceKind::WindowIcons => Task::batch([
                    self.poll_favicons(),
                    self.poll_app_icons(),
                    self.poll_theme_icons(),
                ]),
                SourceKind::DesktopEntries => Self::load_desktop_entries(),
            },
            Message::Battery(msg) => self.battery_group.update(msg),
            Message::System(msg) => self.system_group.update(msg, self.now),
            Message::PanelSettings(msg) => self.update_panel_settings(msg),
            Message::Clock(msg) => self.clock_group.update(msg),
            Message::CloseMenus => Task::batch([
                self.system_group.update(SystemMessage::Close, self.now),
                self.update_panel_settings(PanelSettingsMessage::Close),
            ]),
            Message::FocusWindow(window) => niri_action(move || niri::focus_window(window)),
            Message::FocusWorkspace(workspace) => {
                niri_action(move || niri::focus_workspace(workspace))
            }
            Message::Niri(event) => {
                let window_change = matches!(
                    event,
                    niri_ipc::Event::WindowsChanged { .. }
                        | niri_ipc::Event::WindowOpenedOrChanged { .. }
                        | niri_ipc::Event::WindowClosed { .. }
                );
                let _ = self.niri.apply(event);
                if window_change {
                    Task::batch([
                        self.poll_favicons(),
                        self.poll_app_icons(),
                        self.poll_theme_icons(),
                    ])
                } else {
                    Task::none()
                }
            }
            Message::WindowClosed(id) => {
                self.surfaces.remove(&id);
                self.tooltip.forget(id);
                Task::none()
            }
            Message::Tooltip(msg) => self.tooltip.update(msg, &self.surfaces, &self.config),
            _ => Task::none(),
        }
    }

    /// Re-resolve favicons for every Firefox window currently open. Firefox's
    /// history/favicon DBs are read synchronously off-thread, keyed by the
    /// brand-stripped page title.
    fn poll_favicons(&self) -> Task<Message> {
        let keys = favicons::firefox_keys(&self.niri);
        Task::perform(
            async move { spawn_blocking(move || favicons::resolve(keys)).await },
            |joined| match joined {
                Ok(map) => Message::FaviconsLoaded(map),
                Err(e) => {
                    eprintln!("favicons: resolve task panicked: {e}");
                    Message::FaviconsLoaded(HashMap::new())
                }
            },
        )
    }

    /// Re-resolve `.desktop`-entry app-id icons for every window currently
    /// open, against the last-loaded desktop-entry index. A no-op until that
    /// index has loaded.
    fn poll_app_icons(&self) -> Task<Message> {
        let Some(entries) = self.desktop_entries.clone() else {
            return Task::none();
        };
        let ids = app_icon::app_icon_keys(&self.niri);
        Task::perform(
            async move { spawn_blocking(move || app_icon::resolve_app_icons(&entries, ids)).await },
            |joined| match joined {
                Ok(map) => Message::AppIconsResolved(map),
                Err(e) => {
                    eprintln!("app icons: resolve task panicked: {e}");
                    Message::AppIconsResolved(HashMap::new())
                }
            },
        )
    }

    /// Re-resolve icon-theme-name icons (e.g. a terminal's command-word
    /// icon) for every window currently open. A no-op until the desktop-entry
    /// index has loaded.
    fn poll_theme_icons(&self) -> Task<Message> {
        let Some(entries) = self.desktop_entries.clone() else {
            return Task::none();
        };
        let names = app_icon::theme_icon_keys(&self.niri);
        Task::perform(
            async move { spawn_blocking(move || app_icon::resolve_theme_icons(&entries, names)).await },
            |joined| match joined {
                Ok(map) => Message::ThemeIconsResolved(map),
                Err(e) => {
                    eprintln!("theme icons: resolve task panicked: {e}");
                    Message::ThemeIconsResolved(HashMap::new())
                }
            },
        )
    }

    /// Config-mutating knobs live here rather than in the panel-settings group
    /// itself: that group owns only its expand state, while the config and the
    /// open-surface table (needed to re-margin open surfaces) both belong to
    /// the application.
    fn update_panel_settings(&mut self, msg: PanelSettingsMessage) -> Task<Message> {
        match msg {
            PanelSettingsMessage::Toggle => {
                self.panel_settings_group.menu.toggle(self.now);
                Task::none()
            }
            PanelSettingsMessage::Hover(hovered) => {
                self.panel_settings_group.menu.set_hover(hovered);
                Task::none()
            }
            PanelSettingsMessage::Close => {
                self.panel_settings_group.menu.close(self.now);
                Task::none()
            }
            PanelSettingsMessage::ToggleFloating => {
                self.config.layout.floating = !self.config.layout.floating;
                self.remargin_surfaces()
            }
            PanelSettingsMessage::SetFloatOffset(offset) => {
                self.config.layout.float_offset_px = offset;
                self.remargin_surfaces()
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
