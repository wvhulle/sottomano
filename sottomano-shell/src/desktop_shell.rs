mod compositor_update_handling;
mod config_adoption;
pub mod desktop_panels;
mod group_actions;
pub mod hover_card_updates;
pub mod monitor_layout;
pub mod monitor_surfaces;
pub mod notification_updates;
pub mod pointer_drag;
mod shell_widget_group_readings;
mod shell_widget_group_subscriptions;
pub mod shell_widget_groups;
mod sqlite_app_launches;
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
use sottomano_desktop_sinks::{
    compositor_window_requests::compositor_requests::CompositorRequest,
    desktop_notifications::alert_sound_player::SoundTheme,
};
use sottomano_desktop_sources::{
    blocking_background_work::blocking,
    compositor_window_ipc::{
        compositor_output_facts,
        compositor_window_state::CompositorWindowState,
        niri_kdl_config::{ConfiguredWorkspaces, NiriConfigFile},
    },
    desktop_windows_apps::{
        app_launch_history::LaunchTracker, desktop_entry_changes, workspace_window_snapshot,
    },
    system_protocol_clients::dbus_desktop_buses::{DesktopBusEvent, DesktopBuses},
};
use sottomano_toml_config::{Config, ConfigFile, LoadError, SecretPaths, screen_edges::Position};
use sottomano_widgets::{
    background_work::{compositor_request, copy_to_clipboard},
    clock::Clock,
    desktop_index::DesktopIndex,
    group_menus::{
        cascading_layout::{
            cascade_row::{CascadeDirection, CascadeOrientation, growth_away_from},
            notched_panel::PanelGrowth,
        },
        group_menu_state::{GroupMenuMessage, GroupMenuTopic},
        group_menu_view::{GroupMenuCascadeSurfaces, InspectionCard},
    },
    hover_cards::{
        HoverCardHost, HoverTarget, hover_card_body::hover_card_source_delivery::HoverCard,
        hover_card_surfaces::HoverContent, hover_card_transition::CardRetention,
    },
    notification_toasts::{NotificationLayer, ToastSurfaceStyle},
    widget_groups::{
        appearance_presets_group::AppearancePresetsMessage, idle_pause_groups::IdlePauseMessage,
        niri_window_group::niri_kdl_rules::NiriRulesFile, session_power_group,
    },
    widgets::{
        configured_theme::{configured_theme, text_color},
        favicon::Favicons,
        text::point_generic_font_families_at,
        typing_under_cards::{CardBackgroundFade, TypingUnderCards},
        window_icon::WindowAssets,
    },
};

use self::notification_updates::GroupAlertScan;
use crate::{
    desktop_shell::{
        compositor_update_handling::{
            SwitchHeatEffect, report_missing_compositor, switch_heat_effect,
        },
        monitor_surfaces::{Surface, SurfaceRole, query_outputs},
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
    pub compositor: CompositorWindowState,
    pub window_snapshot: workspace_window_snapshot::Snapshot,
    pub outputs: HashMap<String, compositor_output_facts::OutputInfo>,
    pub surfaces: BTreeMap<Id, Surface>,
    pub backdrops: BTreeMap<Id, Position>,
    pub sidebar_widths: HashMap<Id, f32>,
    pub hover_card: HoverCardHost,
    pub notifications: NotificationLayer<Message>,
    pub desktop_entries: DesktopIndex,
    pub favicons: Favicons,
    pub controller_artwork: ControllerArtwork,
    pub pointer_drag: ShellDrag,
    pub typing_under_cards: TypingUnderCards,
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
        point_generic_font_families_at(&config.theme);
        let clock = Clock::real();
        let shell = Self {
            widget_groups: ShellWidgetGroups::new(&config, secrets, niri_rules, clock),
            launch_tracker: LaunchTracker::default(),
            compositor: CompositorWindowState::default(),
            window_snapshot: workspace_window_snapshot::Snapshot::default(),
            outputs: HashMap::new(),
            surfaces: BTreeMap::new(),
            backdrops: BTreeMap::new(),
            sidebar_widths: HashMap::new(),
            hover_card: HoverCardHost::default(),
            notifications: NotificationLayer::default(),
            desktop_entries: DesktopIndex::default(),
            favicons: Favicons::default(),
            controller_artwork: ControllerArtwork::decoded(),
            pointer_drag: ShellDrag::Idle,
            typing_under_cards: TypingUnderCards::new(clock.instant, &config.overlay_surfaces),
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
            shell.read_niri_workspace_order(),
            report_missing_compositor(),
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
        let hover_card_target_before = self.hover_card.target().cloned();
        let hovered_before = self.hovered_reading_target().cloned();
        let heat_effect = switch_heat_effect(&message);
        let dispatched = self.dispatch_message(message);
        self.widget_groups
            .session_focus_group
            .refresh_daily_totals(&self.config.widget_groups.session_focus, self.clock);
        let reheated = if heat_effect == SwitchHeatEffect::Reheats {
            self.widget_groups
                .reheat_switch_heat(&self.config, &self.compositor, self.clock)
        } else {
            Task::none()
        };
        if self.hover_card.target() != hover_card_target_before.as_ref() {
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
        let open_menu = self.open_group_menu().map(|topic| {
            self.group_menu_of(topic, self.hovered_output())
                .open_group_menu()
        });
        self.widget_groups.group_menus.track_source_demand(
            open_menu,
            promoted_groups,
            self.clock.instant,
        );
        let widget_toasts = self.notifications.show_widget_toasts(
            toast_requests,
            &self.widget_groups.toast_context(
                &self.compositor,
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
            Message::AppLauncher(launcher_message) => self.update_app_launcher(launcher_message),
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
            Message::AppearancePresets(AppearancePresetsMessage::Apply(preset)) => {
                self.widget_groups.apply_appearance_preset(preset)
            }
            Message::AppearancePresets(AppearancePresetsMessage::Expansion(expansion)) => {
                self.widget_groups
                    .appearance_presets_group
                    .update_expansion(expansion);
                Task::none()
            }
            Message::Media(msg) => self.update_media(msg),
            Message::Tray(msg) => self.update_tray(msg),
            Message::VirtualMachines(msg) => self.update_virtual_machines(msg),
            Message::IdleTimeouts(IdlePauseMessage::ScreenPowerChangedByIdleAction(edge)) => self
                .widget_groups
                .session_focus_group
                .screen_power_changed(edge, &self.config.widget_groups.session_focus, self.clock)
                .map(Message::SessionFocus),
            Message::IdleTimeouts(msg) => self
                .widget_groups
                .idle_pause_groups
                .update(msg, self.clock)
                .map(Message::IdleTimeouts),
            Message::GroupMenuState(msg) => self
                .widget_groups
                .group_menus
                .update(msg)
                .map(Message::GroupMenuState),
            Message::UnpinFromPanel { menu, entry } => Task::batch([
                self.dismiss_hover_card(),
                self.widget_groups
                    .group_menus
                    .update(GroupMenuMessage::TogglePin { menu, entry })
                    .map(Message::GroupMenuState),
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
            Message::Compositor(update) => self.apply_compositor_update(update),
            Message::WindowClosed(window) => self.forget_window(window),
            Message::SidebarWidthChanged { window, width } => {
                self.update_sidebar_width(window, width)
            }
            Message::HoverCard(msg) => self.update_hover_card(msg),
            Message::PointerLeft => {
                self.pointer_drag = ShellDrag::Idle;
                self.leave_hover_card()
            }
            Message::SurfacePressed(surface) => {
                let on_toast = self
                    .surfaces
                    .get(&surface)
                    .is_some_and(|host| matches!(host.role, SurfaceRole::NotificationToasts));
                if on_toast && self.hover_card.shown_retention() == CardRetention::UntilDismissed {
                    self.dismiss_hover_card()
                } else {
                    Task::none()
                }
            }
            Message::PointerDrag(msg) => self.update_pointer_drag(msg),
            Message::CompositorRequestFailed(failure) => self.show_compositor_failure(&failure),
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
                        &self.compositor,
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
                self.dismiss_hover_card(),
                compositor_request(CompositorRequest::FocusWindow(window)),
            ]),
            Message::HoverCardHideElapsed => self.leave_hover_card_after_grace(),
            Message::TypingUnderCards(signal) => {
                self.typing_under_cards.update(
                    signal,
                    self.clock.instant,
                    &self.config.overlay_surfaces,
                );
                Task::none()
            }
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
            | Message::Tailnet(_)
            | Message::WireguardTunnels(_)
            | Message::NetworkManagerVpns(_)
            | Message::Monitor(_)
            | Message::ColorTemperature(_)
            | Message::VoiceDictation(_)
            | Message::Audio(_)
            | Message::ClaudeUsage(_)
            | Message::CopilotUsage(_)
            | Message::OpenRouterCredit(_)
            | Message::LlamaServer(_)
            | Message::PodmanEngines(_)
            | Message::NspawnContainers(_)
            | Message::CiRuns(_)
            | Message::SessionFocus(_)
            | Message::LoginSession(_)
            | Message::DarkMode(_)) => {
                self.widget_groups
                    .route(group_message, &self.config, self.clock)
            }
            Message::HoverCardFadeElapsed
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
        if let Some(GroupMenuCascadeSurfaces {
            primary,
            satellites,
        }) = self.group_menu_surfaces(target)
        {
            return Some(HoverContent::GroupMenu {
                primary,
                satellites,
                orientation: self.group_menu_orientation(),
            });
        }
        match self.interactive_panel(target) {
            Some(panel) => Some(HoverContent::Interactive {
                described: self.hover_card_for(target).unwrap_or_else(HoverCard::empty),
                panel,
            }),
            None => self.hover_card_for(target).map(HoverContent::Described),
        }
    }

    fn group_menu_orientation(&self) -> CascadeOrientation {
        let Some((placement, anchor)) = self
            .hover_card
            .placement()
            .zip(self.hover_card.shown_anchor())
        else {
            return CascadeOrientation {
                direction: CascadeDirection::Rightward,
                growth: PanelGrowth::Downward,
            };
        };
        let output_width = self
            .outputs
            .get(&placement.output)
            .map_or(0.0, |output| output.logical_width as f32);
        CascadeOrientation::toward_room(anchor, growth_away_from(placement.position), output_width)
    }

    pub(super) fn hovered_output(&self) -> &str {
        self.hover_card
            .placement()
            .map_or("", |placement| placement.output.as_str())
    }

    fn open_group_menu(&self) -> Option<GroupMenuTopic> {
        match self.hover_card.target() {
            Some(HoverTarget::GroupMenu(topic)) => Some(*topic),
            _ => None,
        }
    }

    fn group_menu_surfaces(
        &self,
        target: &HoverTarget,
    ) -> Option<GroupMenuCascadeSurfaces<'_, Message>> {
        let HoverTarget::GroupMenu(topic) = target else {
            return None;
        };
        let group_menu = self.group_menu_of(*topic, self.hovered_output());
        let inspected_hover_card = self
            .widget_groups
            .group_menus
            .inspected_target(*topic)
            .and_then(|target| {
                self.hover_card_for(target).map(|hover_card| {
                    hover_card
                        .with_announcement(group_menu.announcement_of(target).unwrap_or_default())
                })
            })
            .unwrap_or_else(HoverCard::empty);
        let satellite_fade_level = self.hover_card_output().map_or(0.0, |output| {
            self.card_background_fade(output).fade_level_anywhere()
        });
        group_menu.surfaces(
            &self.config,
            &self
                .widget_groups
                .group_menus
                .menu_appearance()
                .with_background_faded(satellite_fade_level),
            &InspectionCard {
                hover_card: &inspected_hover_card,
                card_appearance: self
                    .widget_groups
                    .overlay_settings_group
                    .overlay_appearance()
                    .with_background_faded(satellite_fade_level),
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
            HoverTarget::Session(session_power_group::SessionTarget::Menu) => Some(
                self.widget_groups
                    .session_power_group
                    .menu_panel(
                        &self.config,
                        popup_corners,
                        self.widget_groups.idle_pause_groups.timeouts(),
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

    fn card_background_fade(&self, output: &str) -> CardBackgroundFade {
        self.typing_under_cards.background_fade_on(
            output,
            &self.compositor,
            self.clock.instant,
            &self.config.overlay_surfaces,
        )
    }

    fn hover_card_layer<'a>(&'a self, output: &str) -> Element<'a, Message> {
        self.hover_card.view(
            output,
            &self.config,
            self.widget_groups
                .hover_card_surface_style(self.card_background_fade(output)),
            self.window_assets(),
            |target: &HoverTarget| {
                self.hover_content(target)
                    .unwrap_or_else(|| HoverContent::Described(HoverCard::empty()))
            },
            self.clock,
        )
    }

    pub fn view(&self, window: Id) -> Element<'_, Message> {
        let _span = tracing::trace_span!("view", ?window).entered();
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
                ..
            }) => self.monitor_layout(window, output),
            Some(Surface {
                output,
                role: SurfaceRole::NotificationToasts,
                ..
            }) => Element::from(
                self.notifications.anchored_toasts(
                    output,
                    &self.config,
                    &self.theme,
                    ToastSurfaceStyle {
                        overlay_appearance: self
                            .widget_groups
                            .overlay_settings_group
                            .overlay_appearance(),
                        panel_clearance: self.panel_clearance(output),
                        background_fade: self.card_background_fade(output),
                    },
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
