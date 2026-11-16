use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use corredo::{
    popovers::{CascadeDirection, CascadeOrientation, PopupGrowth},
    theming::text_color,
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
    compositor_window_requests::compositor_rule_files::CompositorRulesFile,
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
    clock::Clock,
    desktop_index::DesktopIndex,
    group_indicators::group_indicator::HoverFetch,
    group_menus::{
        cascading_layout::popup_growth::growth_away_from,
        group_menu_state::{GroupMenuMessage, GroupMenuTopic, OpenGroupMenu},
        group_menu_view::{GroupMenuCascadeSurfaces, InspectionCard},
    },
    hover_cards::{
        HoverCardHost, HoverTarget, hover_card_surfaces::HoverContent,
        hover_card_transition::CardRetention,
    },
    load_tally::{WorkKind, WorkMeasurement, record_update, record_view},
    notification_toasts::{
        NotificationLayer, ToastSurfaceStyle, toast_updates::StoppedChangeWatch,
    },
    widget_groups::{
        idle_pause_groups::IdlePauseMessage,
        widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity},
        widget_group_interaction::WidgetGroupInteraction,
    },
    widgets::{
        configured_theme::configured_theme,
        favicon::Favicons,
        text::point_generic_font_families_at,
        typing_under_cards::{CardBackgroundFade, TypingUnderCards},
        window_icon::WindowAssets,
    },
};

use crate::{
    compositor_updates::{SwitchHeatEffect, report_missing_compositor, switch_heat_effect},
    monitors::monitor_surfaces::{Surface, SurfaceRole, query_outputs},
    notification_updates::{AlertRescanNeed, GroupAlertScan, alert_rescan_need},
    pointer_drag::ShellDrag,
    shell_message::{Message, ShellInteraction},
    sqlite_app_launches,
    widget_groups::ShellWidgetGroups,
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
    pub active_hover_fetch: HoverFetch,
    pub notifications: NotificationLayer<ShellInteraction>,
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
        compositor_rules: CompositorRulesFile,
        niri_config: NiriConfigFile,
        loaded: Result<Box<Config>, Arc<LoadError>>,
    ) -> (Self, Task<Message>) {
        let _span = tracing::debug_span!("desktop shell init").entered();
        point_generic_font_families_at(&config.theme);
        let clock = Clock::real();
        let shell = Self {
            widget_groups: ShellWidgetGroups::new(&config, secrets, compositor_rules, clock),
            launch_tracker: LaunchTracker::default(),
            compositor: CompositorWindowState::default(),
            window_snapshot: workspace_window_snapshot::Snapshot::default(),
            outputs: HashMap::new(),
            surfaces: BTreeMap::new(),
            backdrops: BTreeMap::new(),
            sidebar_widths: HashMap::new(),
            hover_card: HoverCardHost::default(),
            active_hover_fetch: HoverFetch::NothingToFetch,
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
        let kind = message_work_kind(&message);
        let _span = tracing::trace_span!("update", kind = %kind).entered();
        let measurement = WorkMeasurement::start();
        let updated = self.measured_update(message);
        record_update(kind, measurement.finish());
        updated
    }

    fn measured_update(&mut self, message: Message) -> Task<Message> {
        self.clock = Clock::real();
        let hover_card_target_before = self.hover_card.target().cloned();
        let hovered_before = self.hovered_indicator_target();
        let heat_effect = switch_heat_effect(&message);
        let alert_rescan = alert_rescan_need(&message);
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
        let hover_indicators = if self.hovered_indicator_target() == hovered_before {
            Task::none()
        } else {
            self.retarget_hover_indicators()
        };
        let open_menu = self.open_group_menu().map(|topic| {
            self.group_menu_of(topic, self.hovered_output())
                .open_group_menu()
        });
        let widget_toasts = match alert_rescan {
            AlertRescanNeed::Rescans => self.rescan_group_alerts(open_menu),
            AlertRescanNeed::Skips => {
                self.widget_groups
                    .group_menus
                    .track_open_menu(open_menu, self.clock.instant);
                Task::none()
            }
        };
        let overlay_surfaces = self.reconcile_overlay_surfaces();
        Task::batch([
            dispatched,
            reheated,
            hover_indicators,
            widget_toasts,
            overlay_surfaces,
        ])
    }

    fn rescan_group_alerts(&mut self, open_menu: Option<OpenGroupMenu>) -> Task<Message> {
        let GroupAlertScan {
            toast_requests,
            panel_raises,
        } = self.scan_group_alerts();
        self.widget_groups.group_menus.track_source_demand(
            open_menu,
            panel_raises,
            self.clock.instant,
        );
        self.notifications
            .show_widget_toasts(
                toast_requests,
                &self.widget_groups.toast_context(
                    &self.compositor,
                    self.desktop_entries.get(),
                    &self.config,
                    self.clock,
                ),
            )
            .map(Message::from)
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn dispatch_message(&mut self, message: Message) -> Task<Message> {
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
            Message::FirefoxProfileChanged(Ok(())) => self
                .favicons
                .retry_unresolved()
                .map(Message::FaviconsResolved),
            Message::FirefoxProfileChanged(Err(watch_failure)) => {
                self.show_change_watch_failure(StoppedChangeWatch::FirefoxProfile, &watch_failure)
            }
            Message::Gamepad(msg) => self.update_gamepad(msg),
            Message::Notification(msg) => self.update_notification(msg),
            Message::NotificationServerFailed(server_failure) => {
                tracing::error!("notifications: server stopped, restarting: {server_failure}");
                Task::none()
            }
            Message::PanelSettings(msg) => self.update_panel_settings(msg),
            Message::OverlaySettings(msg) => self.update_overlay_settings(msg),
            Message::NotificationGroup(msg) => self.update_notification_group(msg),
            Message::QuietSchedule(msg) => self.update_quiet_schedule(msg),
            Message::Session(msg) => self.update_session(msg),
            Message::Media(msg) => self.update_media(msg),
            Message::Tray(msg) => self.update_tray(msg),
            Message::GlobalMenu(msg) => self.update_global_menu(msg),
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
            Message::GroupMenuState(msg) => self.update_group_menus(msg),
            Message::Interaction(interaction) => self.interact(interaction),
            Message::SoundThemeLoaded(theme) => {
                self.notifications.adopt_sound_theme(theme);
                Task::none()
            }
            Message::ConfigFileChanged(Ok(())) => self.reload_config(),
            Message::ConfigFileChanged(Err(watch_failure)) => {
                self.show_change_watch_failure(StoppedChangeWatch::ConfigFile, &watch_failure)
            }
            Message::ConfigLoaded(Ok(config)) => self.adopt_config(*config),
            Message::ConfigLoaded(Err(error)) => self.reject_config(&error),
            Message::Compositor(update) => self.apply_compositor_update(update),
            Message::WindowClosed(window) => self.forget_window(window),
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
                self.notifications
                    .show_bus_failure(
                        &failure,
                        &self.widget_groups.toast_context(
                            &self.compositor,
                            self.desktop_entries.get(),
                            &self.config,
                            self.clock,
                        ),
                    )
                    .map(Message::from)
            }
            Message::NiriConfigRead(Ok(configured)) => {
                self.configured_workspaces = Some(configured);
                Task::none()
            }
            Message::NiriConfigRead(Err(reason)) => {
                tracing::warn!("niri config: {reason}");
                Task::none()
            }
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
                self.widget_groups
                    .window_group
                    .settle_workspace_slides(self.clock.instant);
                Task::none()
            }
            group_message @ (Message::Weather(_)
            | Message::Wallpaper(_)
            | Message::Caldav(_)
            | Message::Power(_)
            | Message::Compute(_)
            | Message::MemoryBoard(_)
            | Message::Gpu(_)
            | Message::Storage(_)
            | Message::Drive(_)
            | Message::SidebarSettings(_)
            | Message::CompositorStyling(_)
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
            | Message::CgroupMemoryLimits(_)
            | Message::DockerEngines(_)
            | Message::ApptainerContainers(_)
            | Message::VmwareMachines(_)
            | Message::VirtualBoxMachines(_)
            | Message::WaydroidContainer(_)
            | Message::FlatpakSandboxes(_)
            | Message::LxcContainers(_)
            | Message::IncusInstances(_)
            | Message::MicrovmGuests(_)
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
            | Message::SetForeignParent { .. }
            | Message::ExportToplevel { .. }
            | Message::UnexportToplevel(_)
            | Message::ForgetLastOutput => Task::none(),
        }
    }

    pub(crate) fn hover_content(
        &self,
        target: &HoverTarget,
    ) -> Option<HoverContent<'_, ShellInteraction>> {
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
            Some(panel) => Some(HoverContent::Interactive { panel }),
            None => self
                .described_hover_card(target)
                .map(HoverContent::Described),
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
                growth: PopupGrowth::Downward,
            };
        };
        let output_width = self
            .outputs
            .get(&placement.output)
            .map_or(0.0, |output| output.logical_width as f32);
        CascadeOrientation::toward_room(anchor, growth_away_from(placement.position), output_width)
    }

    pub(crate) fn hovered_output(&self) -> &str {
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
    ) -> Option<GroupMenuCascadeSurfaces<'_, ShellInteraction>> {
        let HoverTarget::GroupMenu(topic) = target else {
            return None;
        };
        let group_menu = self.group_menu_of(*topic, self.hovered_output());
        let inspected_sections = self
            .widget_groups
            .group_menus
            .inspected_indicator(*topic)
            .map(|indicator_key| group_menu.indicator_hover_card(indicator_key, self.clock))
            .unwrap_or_default();
        let satellite_fade_level = self.hover_card_output().map_or(0.0, |output| {
            self.card_background_fade(output).fade_level_anywhere()
        });
        group_menu
            .surfaces(
                &self.config,
                &self
                    .widget_groups
                    .group_menus
                    .layer
                    .appearance()
                    .with_background_faded(satellite_fade_level),
                &InspectionCard {
                    sections: &inspected_sections,
                    card_appearance: self
                        .widget_groups
                        .overlay_settings_group
                        .layer
                        .appearance()
                        .with_background_faded(satellite_fade_level),
                    assets: self.window_assets(),
                    clock: self.clock,
                },
                self.group_menu_orientation(),
            )
            .map(|surfaces| GroupMenuCascadeSurfaces {
                primary: surfaces.primary.map(ShellInteraction::from),
                satellites: surfaces
                    .satellites
                    .into_iter()
                    .map(|satellite| satellite.map(ShellInteraction::from))
                    .collect(),
            })
    }

    fn interactive_panel(&self, target: &HoverTarget) -> Option<Element<'_, ShellInteraction>> {
        let popup_corners = Radius::from(
            self.widget_groups
                .overlay_settings_group
                .layer
                .appearance()
                .frame
                .corner_radius_px,
        );
        match target {
            HoverTarget::Indicator(indicator_key)
                if indicator_key.group == WidgetGroupIdentity::sole(WidgetGroupFamily::Tray) =>
            {
                self.widget_groups
                    .tray_group
                    .menu_panel_named(&indicator_key.name, &self.config, popup_corners)
                    .map(|panel| {
                        panel.map(|tray_message| {
                            ShellInteraction::Group(WidgetGroupInteraction::Tray(tray_message))
                        })
                    })
            }
            HoverTarget::SessionMenu => Some(
                self.widget_groups
                    .session_power_group
                    .menu_panel(
                        &self.config,
                        popup_corners,
                        self.widget_groups.idle_pause_groups.timeouts(),
                    )
                    .map(|session_message| {
                        ShellInteraction::Group(WidgetGroupInteraction::Session(session_message))
                    }),
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

    pub(crate) fn hover_card_layer<'a>(&'a self, output: &str) -> Element<'a, ShellInteraction> {
        self.hover_card.view(
            output,
            &self.config,
            self.widget_groups
                .hover_card_surface_style(self.card_background_fade(output)),
            self.window_assets(),
            |target: &HoverTarget| self.hover_content(target),
            self.clock,
        )
    }

    pub fn view(&self, window: Id) -> Element<'_, Message> {
        let measurement = WorkMeasurement::start();
        let viewed = self.interaction_view(window).map(Message::from);
        record_view(measurement.finish());
        viewed
    }

    fn interaction_view(&self, window: Id) -> Element<'_, ShellInteraction> {
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
            }) => Element::from(self.notifications.anchored_toasts(
                output,
                &self.config,
                &self.theme,
                ToastSurfaceStyle {
                    overlay_appearance:
                        self.widget_groups.overlay_settings_group.layer.appearance(),
                    panel_clearance: self.panel_clearance(output),
                    background_fade: self.card_background_fade(output),
                },
                self.clock,
            ))
            .map(ShellInteraction::Notification),
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

fn message_work_kind(message: &Message) -> WorkKind {
    match message {
        Message::Interaction(ShellInteraction::Group(group_interaction)) => WorkKind {
            layer: "GroupInteraction",
            variant: group_interaction.into(),
        },
        Message::Interaction(shell_interaction) => WorkKind {
            layer: "ShellInteraction",
            variant: shell_interaction.into(),
        },
        message => WorkKind {
            layer: "Message",
            variant: message.into(),
        },
    }
}
