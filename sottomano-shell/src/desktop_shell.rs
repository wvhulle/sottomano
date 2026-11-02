pub mod bottom_bar;
mod config_adoption;
mod group_actions;
pub mod monitor_layout;
pub mod monitor_surfaces;
mod niri_event_updates;
pub mod notification_updates;
pub mod panel_bar;
pub mod pointer_drag;
mod shell_widget_group_readings;
mod shell_widget_group_subscriptions;
pub mod shell_widget_groups;
mod sqlite_app_launches;
pub mod tooltip_updates;
pub mod top_bar;
pub mod workspace_sidebar;

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use iced::{
    Color, Element, Length, Task,
    border::Radius,
    task::Handle,
    theme::Style,
    widget::{Space, container},
    window::Id,
};
use iced_controller_diagram::ControllerArtwork;
use niri_ipc::state::EventStreamState;
use sottomano_config::{Config, ConfigFile, LoadError, SecretPaths, screen_edges::Position};
use sottomano_sources::{
    blocking_background_work::blocking,
    desktop_notifications::alert_sound_player::SoundTheme,
    desktop_windows_apps::{
        app_launch_history::LaunchTracker,
        desktop_entry_changes,
        niri_kdl_config::{ConfiguredWorkspaces, NiriConfigFile},
        niri_output_facts, niri_window_snapshot,
    },
    system_protocol_clients::dbus_desktop_buses::{DesktopBusEvent, DesktopBuses},
};
use sottomano_widgets::{
    background_work::{copy_to_clipboard, niri_action},
    clock::Clock,
    desktop_index::DesktopIndex,
    hover_tooltips::{GroupHandle, HoverTarget, TooltipHost, tooltip_surfaces::HoverContent},
    notification_toasts::NotificationLayer,
    widget_groups::{
        appearance_presets_group::{AppearancePresetsMessage, AppearancePresetsTarget},
        group_menus::{GroupMenuKind, GroupMenuMessage},
        idle_timeouts_group::IdleTimeoutsMessage,
        niri_window_group::niri_kdl_rules::NiriRulesFile,
        session_menu::{self},
    },
    widgets::{
        cascade_row::{CascadeDirection, CascadeOrientation},
        configured_theme::{configured_theme, text_color},
        favicon::Favicons,
        group_menu::{GroupMenuSurfaces, InspectionCard},
        notched_panel::PanelGrowth,
        text::FontFaces,
        tooltip_body::tooltip_source_delivery::Tooltip,
        window_icon::WindowAssets,
    },
};

pub(crate) use self::niri_event_updates::gamepad_window_claim;
use self::notification_updates::GroupAlertScan;
use crate::{
    desktop_shell::{
        monitor_surfaces::{Surface, SurfaceRole, query_outputs},
        niri_event_updates::reheats_switch_heat,
        pointer_drag::ShellDrag,
        shell_widget_groups::ShellWidgetGroups,
    },
    shell_message::Message,
};

pub struct DesktopShell {
    pub wayland: Option<wayland_client::Connection>,
    pub desktop_buses: Option<DesktopBuses>,
    pub config_file: ConfigFile,
    pub clock: Clock,
    pub config: Config,
    pub theme: iced::Theme,
    pub widget_groups: ShellWidgetGroups,
    pub launch_tracker: LaunchTracker,
    pub niri: EventStreamState,
    pub niri_snapshot: niri_window_snapshot::Snapshot,
    pub outputs: HashMap<String, niri_output_facts::OutputInfo>,
    pub surfaces: BTreeMap<Id, Surface>,
    pub backdrops: BTreeMap<Id, Position>,
    pub sidebar_widths: HashMap<Id, f32>,
    pub tooltip: TooltipHost,
    pub notifications: NotificationLayer<Message>,
    pub desktop_entries: DesktopIndex,
    pub favicons: Favicons,
    pub controller_artwork: ControllerArtwork,
    pub pointer_drag: ShellDrag,
    pub spring_load: Option<Handle>,
    pub niri_config: NiriConfigFile,
    pub configured_workspaces: Option<ConfiguredWorkspaces>,
}

impl DesktopShell {
    pub fn new(
        wayland: Option<wayland_client::Connection>,
        config: Config,
        config_file: ConfigFile,
        secrets: SecretPaths,
        niri_rules: NiriRulesFile,
        niri_config: NiriConfigFile,
        loaded: Result<Box<Config>, Arc<LoadError>>,
    ) -> (Self, Task<Message>) {
        let _span = tracing::debug_span!("desktop shell init").entered();
        FontFaces::point_generic_families_at(&config.theme);
        let clock = Clock::real();
        let shell = Self {
            widget_groups: ShellWidgetGroups::new(&config, secrets, niri_rules, clock),
            launch_tracker: LaunchTracker::default(),
            niri: EventStreamState::default(),
            niri_snapshot: niri_window_snapshot::Snapshot::default(),
            outputs: HashMap::new(),
            surfaces: BTreeMap::new(),
            backdrops: BTreeMap::new(),
            sidebar_widths: HashMap::new(),
            tooltip: TooltipHost::default(),
            notifications: NotificationLayer::default(),
            desktop_entries: DesktopIndex::default(),
            favicons: Favicons::default(),
            controller_artwork: ControllerArtwork::decoded(),
            pointer_drag: ShellDrag::Idle,
            spring_load: None,
            niri_config,
            configured_workspaces: None,
            theme: configured_theme(&config.theme.palette),
            wayland,
            desktop_buses: None,
            config,
            config_file,
            clock,
        };
        let init = Task::batch([
            Task::done(Message::ConfigLoaded(loaded)),
            Task::perform(desktop_entry_changes::load(), Message::DesktopEntriesLoaded),
            Task::perform(sqlite_app_launches::load(), Message::TrackerLoaded),
            shell.widget_groups.load(clock),
            Task::perform(shell.niri_config.read_workspaces(), Message::NiriConfigRead),
            Task::perform(
                blocking("sound theme", SoundTheme::read_session),
                Message::SoundThemeLoaded,
            ),
            query_outputs(),
        ]);
        (shell, init)
    }

    #[must_use]
    pub fn namespace() -> String {
        "sottomano".to_string()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let _span = tracing::trace_span!("update", ?message).entered();
        self.clock = Clock::real();
        let tooltip_target_before = self.tooltip.target().cloned();
        let hovered_before = self.hovered_reading_target().cloned();
        let reheat_due = reheats_switch_heat(&message);
        let dispatched = self.dispatch_message(message);
        self.widget_groups
            .session_focus_group
            .refresh_daily_totals(&self.config.widget_groups.focus_time, self.clock);
        let reheated = if reheat_due {
            self.widget_groups
                .reheat_switch_heat(&self.config, &self.niri, self.clock)
        } else {
            Task::none()
        };
        if self.tooltip.target() != tooltip_target_before.as_ref() {
            let _closed = self
                .widget_groups
                .group_menus
                .update(GroupMenuMessage::Closed);
        }
        let hover_readings = if self.hovered_reading_target() == hovered_before.as_ref() {
            Task::none()
        } else {
            self.retarget_hover_readings()
        };
        let GroupAlertScan {
            toast_requests,
            promoted_groups,
        } = self.scan_group_alerts();
        let open_menu = self.open_group_menu();
        self.widget_groups.group_menus.track_source_demand(
            open_menu,
            promoted_groups,
            self.clock.instant,
        );
        let widget_toasts = self.notifications.show_widget_toasts(
            toast_requests,
            &self.widget_groups.toast_context(
                &self.niri,
                self.desktop_entries.get(),
                &self.config,
                self.clock,
            ),
        );
        let overlay_surfaces = self.reconcile_overlay_surfaces();
        Task::batch([
            dispatched,
            reheated,
            hover_readings,
            widget_toasts,
            overlay_surfaces,
        ])
    }

    #[allow(clippy::too_many_lines)]
    fn dispatch_message(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Output(change) => self.apply_output_change(change),
            Message::WindowGroup(msg) => self.update_window_group(msg),
            Message::TrackerLoaded(launches) => self.load_launch_tracker(launches),
            Message::DesktopEntriesLoaded(entries) => self.load_desktop_entries(entries),
            Message::ReloadDesktopEntries => {
                Task::perform(desktop_entry_changes::load(), Message::DesktopEntriesLoaded)
            }
            Message::IconsResolved(resolved) => {
                self.desktop_entries.merge(resolved);
                Task::none()
            }
            Message::FaviconsResolved(raw) => {
                self.favicons.merge(raw).map(Message::FaviconsResolved)
            }
            Message::FirefoxProfileChanged => self
                .favicons
                .retry_unresolved()
                .map(Message::FaviconsResolved),
            Message::Gamepad(msg) => self.update_gamepad(msg),
            Message::Notification(msg) => self.update_notification(msg),
            Message::PanelSettings(msg) => self.update_panel_settings(msg),
            Message::OverlaySettings(msg) => self.update_overlay_settings(msg),
            Message::NotificationGroup(msg) => self.update_notification_group(msg),
            Message::QuietSchedule(msg) => self.update_quiet_schedule(msg),
            Message::Session(msg) => self.update_session(msg),
            Message::AppearancePresets(AppearancePresetsMessage::Apply(preset)) => Task::batch([
                self.dismiss_tooltip(),
                self.widget_groups.apply_appearance_preset(preset),
            ]),
            Message::Media(msg) => self.update_media(msg),
            Message::Tray(msg) => self.update_tray(msg),
            Message::IdleTimeouts(IdleTimeoutsMessage::ScreenPowerChangedByIdleAction(edge)) => {
                self.widget_groups
                    .session_focus_group
                    .screen_power_changed(edge, &self.config.widget_groups.focus_time, self.clock)
                    .map(Message::SessionFocus)
            }
            Message::IdleTimeouts(msg) => self
                .widget_groups
                .idle_timeouts_group
                .update(msg)
                .map(Message::IdleTimeouts),
            Message::GroupMenus(msg) => self
                .widget_groups
                .group_menus
                .update(msg)
                .map(Message::GroupMenus),
            Message::UnpinFromPanel { menu, entry } => Task::batch([
                self.dismiss_tooltip(),
                self.widget_groups
                    .group_menus
                    .update(GroupMenuMessage::TogglePin { menu, entry })
                    .map(Message::GroupMenus),
            ]),
            Message::SoundThemeLoaded(theme) => {
                self.notifications.adopt_sound_theme(theme);
                Task::none()
            }
            Message::ConfigFileChanged => self.reload_config(),
            Message::ConfigLoaded(Ok(config)) => self.adopt_config(*config),
            Message::ConfigLoaded(Err(error)) => self.reject_config(&error),
            Message::DismissPressed => {
                self.widget_groups.collapse_when_auto_collapsing();
                Task::none()
            }
            Message::Niri(events) => self.apply_niri_events(events),
            Message::WindowClosed(window) => self.forget_window(window),
            Message::SidebarWidthChanged { window, width } => {
                self.update_sidebar_width(window, width)
            }
            Message::Tooltip(msg) => self.update_tooltip(msg),
            Message::PointerLeft => {
                self.pointer_drag = ShellDrag::Idle;
                self.leave_tooltip()
            }
            Message::PointerDrag(msg) => self.update_pointer_drag(msg),
            Message::NiriActionFailed(failure) => self.show_niri_failure(&failure),
            Message::DesktopBus(Ok(DesktopBusEvent::Connected(buses))) => {
                self.desktop_buses = Some(buses);
                Task::none()
            }
            Message::DesktopBus(Ok(DesktopBusEvent::Disconnected)) => {
                tracing::warn!("desktop buses: connection closed, reconnecting");
                self.desktop_buses = None;
                Task::none()
            }
            Message::DesktopBus(Err(failure)) => {
                tracing::error!("desktop buses: {failure}");
                self.notifications.show_bus_failure(
                    &failure,
                    &self.widget_groups.toast_context(
                        &self.niri,
                        self.desktop_entries.get(),
                        &self.config,
                        self.clock,
                    ),
                )
            }
            Message::NiriConfigRead(Ok(configured)) => {
                self.configured_workspaces = Some(configured);
                Task::none()
            }
            Message::NiriConfigRead(Err(reason)) => {
                tracing::warn!("niri config: {reason}");
                Task::none()
            }
            Message::RestoreWorkspaceOrder { output } => self.restore_workspace_order(&output),
            Message::CopyText(text) => copy_to_clipboard(text),
            Message::OpenUrl(url) => self.open_url(url),
            Message::RefreshHoveredReading => Task::batch([
                self.refresh_hovered_reading(),
                self.retarget_hover_readings(),
            ]),
            Message::FocusWindow(window) => Task::batch([
                self.dismiss_tooltip(),
                niri_action(niri_ipc::Action::FocusWindow { id: window }),
            ]),
            Message::TooltipHideElapsed => self.leave_tooltip_after_grace(),
            Message::OutputsResolved(outputs) => self.adopt_outputs(outputs),
            Message::Redraw => {
                self.notifications.prune(self.clock.instant);
                Task::none()
            }
            group_message @ (Message::Weather(_)
            | Message::Wallpaper(_)
            | Message::Caldav(_)
            | Message::Power(_)
            | Message::Compute(_)
            | Message::MemoryBoard(_)
            | Message::HostHardware(_)
            | Message::Gpu(_)
            | Message::Storage(_)
            | Message::Drive(_)
            | Message::SidebarSettings(_)
            | Message::NiriWindow(_)
            | Message::Clock(_)
            | Message::SystemdManager(_)
            | Message::JournalEvents(_)
            | Message::PendingUpgrades(_)
            | Message::WifiAdapters(_)
            | Message::MobileModems(_)
            | Message::EthernetPorts(_)
            | Message::BluetoothAdapters(_)
            | Message::Reachability(_)
            | Message::Vpn(_)
            | Message::Monitor(_)
            | Message::ColorTemperature(_)
            | Message::VoiceDictation(_)
            | Message::Audio(_)
            | Message::ClaudeUsage(_)
            | Message::CopilotUsage(_)
            | Message::OpenRouterCredit(_)
            | Message::LlamaServer(_)
            | Message::SessionFocus(_)
            | Message::LoginSession(_)
            | Message::DarkMode(_)) => {
                self.widget_groups
                    .route(group_message, &self.config, self.clock)
            }
            Message::TooltipFadeElapsed
            | Message::LayoutChange { .. }
            | Message::BlurOptionChange { .. }
            | Message::PopUpReposition { .. }
            | Message::SetInputRegion { .. }
            | Message::RequestActivationToken { .. }
            | Message::LayerChange { .. }
            | Message::MarginChange { .. }
            | Message::ExclusiveZoneChange { .. }
            | Message::KeyboardInteractivityChange { .. }
            | Message::VirtualKeyboardPressed { .. }
            | Message::NewLayerShell { .. }
            | Message::NewBaseWindow { .. }
            | Message::NewPopUp { .. }
            | Message::NewMenu { .. }
            | Message::NewInputPanel { .. }
            | Message::RemoveWindow(_)
            | Message::RedrawWindow(_)
            | Message::ForgetLastOutput => Task::none(),
        }
    }

    pub(super) fn hover_content(&self, target: &HoverTarget) -> Option<HoverContent<'_, Message>> {
        if let Some(GroupMenuSurfaces {
            primary,
            satellites,
        }) = self.group_menu_surfaces(target)
        {
            return Some(HoverContent::Cascade {
                primary,
                satellites,
                orientation: self.group_menu_orientation(),
            });
        }
        match self.interactive_panel(target) {
            Some(panel) => Some(HoverContent::Interactive {
                described: self.tooltip_for(target).unwrap_or_else(Tooltip::empty),
                panel,
            }),
            None => self.tooltip_for(target).map(HoverContent::Described),
        }
    }

    fn group_menu_orientation(&self) -> CascadeOrientation {
        let Some(placement) = self.tooltip.placement() else {
            return CascadeOrientation {
                direction: CascadeDirection::Rightward,
                growth: PanelGrowth::Downward,
            };
        };
        let output_width = self
            .outputs
            .get(&placement.output)
            .map_or(0.0, |output| output.logical_width as f32);
        CascadeOrientation::toward_room(placement.anchor, placement.position, output_width)
    }

    fn open_group_menu(&self) -> Option<GroupMenuKind> {
        match self.tooltip.target() {
            Some(HoverTarget::GroupHandle(GroupHandle::GroupMenu(kind))) => Some(*kind),
            _ => None,
        }
    }

    fn group_menu_surfaces(&self, target: &HoverTarget) -> Option<GroupMenuSurfaces<'_, Message>> {
        let HoverTarget::GroupHandle(GroupHandle::GroupMenu(kind)) = target else {
            return None;
        };
        let group_menu = self
            .bar_group_menus()
            .find(|group_menu| group_menu.kind() == *kind)?;
        let card_tooltip = self
            .widget_groups
            .group_menus
            .inspected_row(*kind)
            .and_then(|row| {
                self.tooltip_for(&row.target)
                    .map(|tooltip| tooltip.with_announcement(row.announcement.clone()))
            })
            .unwrap_or_else(Tooltip::empty);
        group_menu.surfaces(
            &self.config,
            &self
                .widget_groups
                .overlay_settings_group
                .overlay_appearance(),
            &InspectionCard {
                tooltip: &card_tooltip,
                assets: self.window_assets(),
                clock: self.clock,
            },
            self.group_menu_orientation(),
        )
    }

    fn interactive_panel(&self, target: &HoverTarget) -> Option<Element<'_, Message>> {
        let popup_corners = Radius::from(
            self.widget_groups
                .overlay_settings_group
                .overlay_appearance()
                .corner_radius_px,
        );
        match target {
            HoverTarget::Tray(address) => self
                .widget_groups
                .tray_group
                .menu_panel(address, &self.config, popup_corners)
                .map(|panel| panel.map(Message::Tray)),
            HoverTarget::AppearancePresets(AppearancePresetsTarget::Menu) => Some(
                self.widget_groups
                    .appearance_presets_menu
                    .menu_panel(
                        &self.config,
                        popup_corners,
                        &self.widget_groups.shell_appearance(),
                    )
                    .map(Message::AppearancePresets),
            ),
            HoverTarget::Session(session_menu::SessionTarget::Menu) => Some(
                self.widget_groups
                    .session_menu
                    .menu_panel(
                        &self.config,
                        popup_corners,
                        self.widget_groups.idle_timeouts_group.timeouts(),
                    )
                    .map(Message::Session),
            ),
            _ => None,
        }
    }

    #[must_use]
    pub fn window_assets(&self) -> WindowAssets<'_> {
        WindowAssets {
            entries: &self.desktop_entries,
            favicons: &self.favicons,
            controller_artwork: &self.controller_artwork,
        }
    }

    pub fn view(&self, window: Id) -> Element<'_, Message> {
        let _span = tracing::trace_span!("view", ?window).entered();
        if let Some(tooltip) = self.tooltip.view(
            window,
            &self.config,
            self.widget_groups.tooltip_surface_style(),
            self.window_assets(),
            |target: &HoverTarget| {
                self.hover_content(target)
                    .unwrap_or_else(|| HoverContent::Described(Tooltip::empty()))
            },
            self.clock,
        ) {
            return tooltip;
        }
        let blank = || Space::new().width(Length::Fill).height(Length::Fill);
        if self.backdrops.contains_key(&window) {
            return container(blank())
                .style(|_| container::background(Color::BLACK))
                .into();
        }
        match self.surfaces.get(&window) {
            Some(Surface {
                output,
                role: SurfaceRole::Panel,
            }) => self.monitor_layout(window, output),
            Some(Surface {
                output,
                role: SurfaceRole::NotificationToasts,
            }) => Element::from(
                self.notifications.anchored_toasts(
                    output,
                    &self.config,
                    &self.theme,
                    self.widget_groups
                        .overlay_settings_group
                        .overlay_appearance(),
                    self.bar_clearance(output),
                    self.clock,
                ),
            )
            .map(Message::Notification),
            _ => blank().into(),
        }
    }

    #[must_use]
    pub fn theme(&self, _window: Id) -> iced::Theme {
        self.theme.clone()
    }

    #[must_use]
    pub fn style(theme: &iced::Theme) -> Style {
        Style {
            background_color: Color::TRANSPARENT,
            text_color: text_color(theme),
        }
    }
}
