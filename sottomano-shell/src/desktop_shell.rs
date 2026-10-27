pub mod bottom_bar;
pub mod monitor_layout;
pub mod monitor_surfaces;
pub mod notification_updates;
pub mod panel_bar;
pub mod pointer_drag;
mod sqlite_app_launches;
pub mod tooltip_updates;
pub mod top_bar;
pub mod workspace_sidebar;

use std::{
    collections::{BTreeMap, HashMap},
    mem,
    sync::Arc,
};

use iced::{
    Color, Element, Length, Task, font,
    task::Handle,
    theme::Style,
    widget::{Space, container},
    window::Id,
};
use iced_controller_diagram::ControllerArtwork;
use niri_ipc::state::{EventStreamState, EventStreamStatePart};
use sottomano_config::{
    Config, ConfigFile, LoadError, SecretPaths, panel_bar::PanelBarConfig, screen_edges::Position,
    theme::Theme,
};
use sottomano_sources::{
    blocking_background_work::blocking,
    desktop_notifications::alert_sound_player::SoundTheme,
    desktop_windows_apps::{
        app_launch_history::{Launch, LaunchTracker},
        desktop_entry_app_icons::{self, DesktopEntries},
        desktop_entry_changes, niri_action_requests,
        niri_kdl_config::{ConfiguredWorkspaces, NiriConfigFile},
        niri_output_facts, niri_window_placement, niri_window_snapshot,
    },
    system_protocol_clients::{
        dbus_desktop_buses::{DesktopBusEvent, DesktopBuses},
        shell_command_output::blocking::run,
    },
    voice_dictation_input::notulist_dbus_dictation::send_logged_dictation_command,
};
use sottomano_widgets::{
    background_work::{copy_to_clipboard, niri_action},
    clock::Clock,
    desktop_index::DesktopIndex,
    hover_tooltips::{TooltipHost, tooltip_surfaces::TooltipSurfaceStyle},
    notification_toasts::{NotificationLayer, NotificationSettings, ToastContext},
    widget_groups::{
        audio_group::AudioGroup,
        caldav_group::CaldavGroup,
        claude_usage_group::ClaudeUsageGroup,
        clock_group::ClockGroup,
        color_temperature_group::ColorTemperatureGroup,
        compute_group::{ComputeGroup, ComputeMessage},
        copilot_usage_group::CopilotUsageGroup,
        dark_mode_group::DarkModeGroup,
        disk_group::DiskGroup,
        gamepad_group::{self, GamepadGroup, GamepadMessage},
        gpu_group::GpuGroup,
        idle_timeouts_group::{IdleTimeoutsGroup, IdleTimeoutsMessage},
        internet_group::InternetGroup,
        journal_events_group::JournalEventsGroup,
        llama_server_group::LlamaServerGroup,
        login_session_group::LoginSessionGroup,
        media_group::{self, MediaGroup, MediaMessage},
        memory_board_group::{MemoryBoardGroup, MemoryBoardMessage},
        monitor_group::MonitorGroup,
        net_interfaces_group::NetInterfacesGroup,
        niri_window_group::{NiriWindowGroup, niri_kdl_rules::NiriRulesFile},
        notification_group::NotificationGroup,
        openrouter_credit_group::OpenRouterCreditGroup,
        overlay_settings_group::{self, OverlaySettingsGroup, OverlaySettingsMessage},
        panel_settings_group::{self, PanelSettingsGroup, PanelSettingsMessage},
        pending_upgrades_group::PendingUpgradesGroup,
        power_group::PowerGroup,
        quiet_schedule_group::QuietScheduleGroup,
        session_focus_group::SessionFocusGroup,
        session_menu::{self, SessionMenu, SessionMessage},
        sidebar_settings_group::SidebarSettingsGroup,
        systemd_manager_group::SystemdManagerGroup,
        tray_group::{self, TrayGroup, TrayMessage},
        voice_dictation_group::VoiceDictationGroup,
        vpn_group::VpnGroup,
        wallpaper_group::WallpaperGroup,
        weather_group::WeatherGroup,
        window_group::{self, WindowGroup, WindowGroupMessage, window_list},
    },
    widgets::{
        app_window_focus::AppWindowFocus, continuous_reading::ContinuousReading, favicon::Favicons,
        palette_colors::PaletteColors, text::FontFaces, tooltip_body::Tooltip,
        window_icon::WindowAssets,
    },
};

use crate::{
    desktop_shell::{
        monitor_surfaces::{Surface, SurfaceRole, query_outputs},
        pointer_drag::ShellDrag,
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
    pub weather_group: WeatherGroup,
    pub wallpaper_group: WallpaperGroup,
    pub caldav_group: CaldavGroup,
    pub power_group: PowerGroup,
    pub compute_group: ComputeGroup,
    pub memory_board_group: MemoryBoardGroup,
    pub gpu_group: GpuGroup,
    pub disk_group: DiskGroup,
    pub panel_settings_group: PanelSettingsGroup,
    pub sidebar_settings_group: SidebarSettingsGroup,
    pub overlay_settings_group: OverlaySettingsGroup,
    pub niri_window_group: NiriWindowGroup,
    pub clock_group: ClockGroup,
    pub notification_group: NotificationGroup,
    pub quiet_schedule_group: QuietScheduleGroup,
    pub session_menu: SessionMenu,
    pub systemd_manager_group: SystemdManagerGroup,
    pub journal_events_group: JournalEventsGroup,
    pub pending_upgrades_group: PendingUpgradesGroup,
    pub internet_group: InternetGroup,
    pub vpn_group: VpnGroup,
    pub net_interfaces_group: NetInterfacesGroup,
    pub monitor_group: MonitorGroup,
    pub color_temperature_group: ColorTemperatureGroup,
    pub dark_mode_group: DarkModeGroup,
    pub voice_dictation_group: VoiceDictationGroup,
    pub idle_timeouts_group: IdleTimeoutsGroup,
    pub media_group: MediaGroup,
    pub gamepad_group: GamepadGroup,
    pub audio_group: AudioGroup,
    pub claude_usage_group: ClaudeUsageGroup,
    pub copilot_usage_group: CopilotUsageGroup,
    pub openrouter_credit_group: OpenRouterCreditGroup,
    pub llama_server_group: LlamaServerGroup,
    pub session_focus_group: SessionFocusGroup,
    pub login_session_group: LoginSessionGroup,
    pub tray_group: TrayGroup,
    pub window_group: WindowGroup,
    pub launch_tracker: LaunchTracker,
    pub niri: EventStreamState,
    pub niri_snapshot: ContinuousReading<niri_window_snapshot::Snapshot>,
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
            weather_group: WeatherGroup::default(),
            wallpaper_group: WallpaperGroup::new(&secrets),
            caldav_group: CaldavGroup::new(&secrets),
            power_group: PowerGroup::default(),
            compute_group: ComputeGroup::default(),
            memory_board_group: MemoryBoardGroup::default(),
            gpu_group: GpuGroup::default(),
            disk_group: DiskGroup::default(),
            panel_settings_group: PanelSettingsGroup::default(),
            sidebar_settings_group: SidebarSettingsGroup::default(),
            overlay_settings_group: OverlaySettingsGroup::default(),
            niri_window_group: NiriWindowGroup::new(niri_rules),
            clock_group: ClockGroup::default(),
            notification_group: NotificationGroup::default(),
            quiet_schedule_group: QuietScheduleGroup::default(),
            session_menu: SessionMenu::default(),
            systemd_manager_group: SystemdManagerGroup::default(),
            journal_events_group: JournalEventsGroup::default(),
            pending_upgrades_group: PendingUpgradesGroup::default(),
            internet_group: InternetGroup::default(),
            vpn_group: VpnGroup::default(),
            net_interfaces_group: NetInterfacesGroup::default(),
            monitor_group: MonitorGroup::default(),
            color_temperature_group: ColorTemperatureGroup::new(
                config.launched_programs.color_temperature_service.clone(),
            ),
            dark_mode_group: DarkModeGroup::new(clock),
            voice_dictation_group: VoiceDictationGroup::new(),
            idle_timeouts_group: IdleTimeoutsGroup::default(),
            media_group: MediaGroup::default(),
            gamepad_group: GamepadGroup::default(),
            audio_group: AudioGroup::default(),
            claude_usage_group: ClaudeUsageGroup::new(secrets.claude_code_oauth),
            copilot_usage_group: CopilotUsageGroup::new(secrets.gh_auth_token),
            openrouter_credit_group: OpenRouterCreditGroup::new(secrets.openrouter_management),
            llama_server_group: LlamaServerGroup::default(),
            session_focus_group: SessionFocusGroup::started_at(
                &config.widget_groups.focus_time,
                clock.utc(),
            ),
            login_session_group: LoginSessionGroup::default(),
            tray_group: TrayGroup::default(),
            window_group: WindowGroup::default(),
            launch_tracker: LaunchTracker::default(),
            niri: EventStreamState::default(),
            niri_snapshot: ContinuousReading::default(),
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
            theme: iced::Theme::from(&PaletteColors::from(&config.theme.palette)),
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
            SessionFocusGroup::load(clock).map(Message::SessionFocus),
            SessionMenu::load().map(Message::Session),
            IdleTimeoutsGroup::load().map(Message::IdleTimeouts),
            PanelSettingsGroup::load().map(Message::PanelSettings),
            SidebarSettingsGroup::load().map(Message::SidebarSettings),
            OverlaySettingsGroup::load().map(Message::OverlaySettings),
            NotificationGroup::load().map(Message::NotificationGroup),
            QuietScheduleGroup::load().map(Message::QuietSchedule),
            DarkModeGroup::load().map(Message::DarkMode),
            shell.niri_window_group.load().map(Message::NiriWindow),
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
        let tooltip_seq = self.tooltip.seq();
        let hovered_before = self.tooltip.target().cloned();
        let reheat_due = reheats_switch_heat(&message);
        let dispatched = self.dispatch_message(message);
        self.session_focus_group
            .refresh_daily_totals(&self.config.widget_groups.focus_time, self.clock);
        let reheated = if reheat_due {
            self.niri_window_group
                .reheat(&self.config, || {
                    self.session_focus_group
                        .heated_windows(&self.niri, &self.config, self.clock)
                })
                .map(Message::NiriWindow)
        } else {
            Task::none()
        };
        let hover_readings = if self.tooltip.target() == hovered_before.as_ref() {
            Task::none()
        } else {
            self.retarget_hover_readings()
        };
        let requests = self.group_toast_requests();
        let widget_toasts = self.notifications.show_widget_toasts(
            requests,
            &ToastContext::new(
                &self.niri,
                self.desktop_entries.get(),
                &self.config,
                self.notification_settings(),
                self.quiet_schedule_group.is_quiet(),
                self.clock,
            ),
        );
        Task::batch([
            dispatched,
            reheated,
            hover_readings,
            widget_toasts,
            self.redraw_content(tooltip_seq),
        ])
    }

    fn redraw_content(&self, tooltip_seq_before_update: u64) -> Task<Message> {
        Task::batch(
            self.panels()
                .chain(self.notification_toast_surfaces())
                .map(|(surface, _)| surface)
                .chain(self.tooltip.surfaces_to_redraw(tooltip_seq_before_update))
                .map(|surface| Task::done(Message::RedrawWindow(surface))),
        )
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
            Message::Weather(msg) => self
                .weather_group
                .update(msg, self.clock)
                .map(Message::Weather),
            Message::Wallpaper(msg) => self
                .wallpaper_group
                .update(msg, self.clock)
                .map(Message::Wallpaper),
            Message::Caldav(msg) => self
                .caldav_group
                .update(msg, self.clock)
                .map(Message::Caldav),
            Message::Gamepad(msg) => self.update_gamepad(msg),
            Message::Notification(msg) => self.update_notification(msg),
            Message::Power(msg) => self.power_group.update(msg, self.clock).map(Message::Power),
            Message::Compute(msg) => self
                .compute_group
                .update(msg, self.clock)
                .map(Message::Compute),
            Message::MemoryBoard(msg) => self
                .memory_board_group
                .update(msg, self.clock)
                .map(Message::MemoryBoard),
            Message::HostHardware(delivery) => Task::batch([
                self.compute_group
                    .update(ComputeMessage::HostHardware(delivery.clone()), self.clock)
                    .map(Message::Compute),
                self.memory_board_group
                    .update(MemoryBoardMessage::Hardware(delivery), self.clock)
                    .map(Message::MemoryBoard),
            ]),
            Message::Gpu(msg) => self.gpu_group.update(msg, self.clock).map(Message::Gpu),
            Message::Disk(msg) => self.disk_group.update(msg, self.clock).map(Message::Disk),
            Message::PanelSettings(msg) => self.update_panel_settings(msg),
            Message::SidebarSettings(msg) => self
                .sidebar_settings_group
                .update(msg)
                .map(Message::SidebarSettings),
            Message::OverlaySettings(msg) => self.update_overlay_settings(msg),
            Message::NiriWindow(msg) => self.niri_window_group.update(msg).map(Message::NiriWindow),
            Message::Clock(msg) => self.clock_group.update(msg, self.clock).map(Message::Clock),
            Message::NotificationGroup(msg) => self.update_notification_group(msg),
            Message::QuietSchedule(msg) => self.update_quiet_schedule(msg),
            Message::Session(msg) => self.update_session(msg),
            Message::SystemdManager(msg) => self
                .systemd_manager_group
                .update(msg, self.clock)
                .map(Message::SystemdManager),
            Message::JournalEvents(msg) => self
                .journal_events_group
                .update(msg, self.clock)
                .map(Message::JournalEvents),
            Message::PendingUpgrades(msg) => self
                .pending_upgrades_group
                .update(msg, self.clock)
                .map(Message::PendingUpgrades),
            Message::NetInterfaces(msg) => self
                .net_interfaces_group
                .update(msg, self.clock)
                .map(Message::NetInterfaces),
            Message::Internet(msg) => {
                self.internet_group.update(msg, self.clock);
                Task::none()
            }
            Message::Vpn(msg) => self.vpn_group.update(msg, self.clock).map(Message::Vpn),
            Message::Monitor(msg) => self
                .monitor_group
                .update(msg, self.clock)
                .map(Message::Monitor),
            Message::ColorTemperature(msg) => self
                .color_temperature_group
                .update(msg, self.clock)
                .map(Message::ColorTemperature),
            Message::DarkMode(msg) => self
                .dark_mode_group
                .update(msg, self.clock)
                .map(Message::DarkMode),
            Message::VoiceDictation(msg) => self
                .voice_dictation_group
                .update(msg, self.clock)
                .map(Message::VoiceDictation),
            Message::Media(msg) => self.update_media(msg),
            Message::Tray(msg) => self.update_tray(msg),
            Message::IdleTimeouts(IdleTimeoutsMessage::ScreenPowerChangedByIdleAction(edge)) => {
                self.session_focus_group
                    .screen_power_changed(edge, &self.config.widget_groups.focus_time, self.clock)
                    .map(Message::SessionFocus)
            }
            Message::IdleTimeouts(msg) => self
                .idle_timeouts_group
                .update(msg)
                .map(Message::IdleTimeouts),
            Message::SoundThemeLoaded(theme) => {
                self.notifications.adopt_sound_theme(theme);
                Task::none()
            }
            Message::ConfigFileChanged => self.reload_config(),
            Message::ConfigLoaded(Ok(config)) => self.adopt_config(*config),
            Message::ConfigLoaded(Err(error)) => self.reject_config(&error),
            Message::Audio(msg) => self.audio_group.update(msg, self.clock).map(Message::Audio),
            Message::ClaudeUsage(msg) => self
                .claude_usage_group
                .update(msg, self.clock)
                .map(Message::ClaudeUsage),
            Message::CopilotUsage(msg) => self
                .copilot_usage_group
                .update(msg, self.clock)
                .map(Message::CopilotUsage),
            Message::OpenRouterCredit(msg) => self
                .openrouter_credit_group
                .update(msg, self.clock)
                .map(Message::OpenRouterCredit),
            Message::LlamaServer(msg) => self
                .llama_server_group
                .update(msg, &self.config, self.clock)
                .map(Message::LlamaServer),
            Message::SessionFocus(msg) => self
                .session_focus_group
                .update(msg, &self.config.widget_groups.focus_time, self.clock)
                .map(Message::SessionFocus),
            Message::LoginSession(msg) => self
                .login_session_group
                .update(msg, self.clock)
                .map(Message::LoginSession),
            Message::DismissPressed => {
                self.collapse_groups();
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
                    &ToastContext::new(
                        &self.niri,
                        self.desktop_entries.get(),
                        &self.config,
                        self.notification_settings(),
                        self.quiet_schedule_group.is_quiet(),
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
            Message::LayoutChange { .. }
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

    fn open_url(&mut self, url: String) -> Task<Message> {
        Task::batch([
            self.dismiss_tooltip(),
            Task::future(blocking("open url", move || run("xdg-open", &[&url]))).discard(),
        ])
    }

    fn update_overlay_settings(&mut self, msg: OverlaySettingsMessage) -> Task<Message> {
        match self.overlay_settings_group.update(msg) {
            overlay_settings_group::Action::Run(task) => task.map(Message::OverlaySettings),
            overlay_settings_group::Action::ReflowToasts(task) => {
                Task::batch([self.reflow_toasts(), task.map(Message::OverlaySettings)])
            }
        }
    }

    fn load_launch_tracker(&mut self, launches: Vec<Launch>) -> Task<Message> {
        self.launch_tracker.load_from(launches);
        self.refresh_quick_launch()
    }

    fn load_desktop_entries(&mut self, entries: Arc<DesktopEntries>) -> Task<Message> {
        self.desktop_entries.set(entries.clone());
        self.window_group.set_desktop_entries(entries);
        self.refresh_quick_launch()
    }

    fn adopt_outputs(
        &mut self,
        outputs: HashMap<String, niri_output_facts::OutputInfo>,
    ) -> Task<Message> {
        self.outputs = outputs;
        self.reflow_toasts()
    }

    fn reflow_toasts(&mut self) -> Task<Message> {
        let placement = niri_window_placement::Placement::derive(&self.niri);
        self.notifications.on_niri_changed(
            &placement,
            &self.config,
            self.notification_settings(),
            self.clock.instant,
        )
    }

    fn refocus_session(&mut self) -> Task<Message> {
        self.session_focus_group
            .refocus(&self.niri, self.window_group.window_processes())
            .map(Message::SessionFocus)
    }

    fn update_window_group(&mut self, window_message: WindowGroupMessage) -> Task<Message> {
        let handled = match self.window_group.update(window_message, self.clock) {
            window_group::Action::Run(task) => task.map(Message::WindowGroup),
            window_group::Action::WindowList(window_list::Action::ToggleAudio(pid)) => {
                self.audio_group.toggle_window_mute(pid).map(Message::Audio)
            }
            window_group::Action::WindowList(window_list::Action::None) => Task::none(),
            window_group::Action::WindowList(window_list::Action::WindowProcessesPaired) => {
                self.derive_niri_snapshot();
                self.resolve_icons()
            }
            window_group::Action::WindowList(window_list::Action::Drag(drag)) => {
                self.update_pointer_drag(drag)
            }
        };
        Task::batch([handled, self.refocus_session()])
    }

    fn update_gamepad(&mut self, msg: GamepadMessage) -> Task<Message> {
        match self.gamepad_group.update(
            msg,
            &self.config.widget_groups.gamepad,
            focused_app_id(&self.niri),
        ) {
            gamepad_group::Action::Idle => Task::none(),
            gamepad_group::Action::Compositor(action) => niri_action(action),
            gamepad_group::Action::Dictation(command) => {
                Task::future(send_logged_dictation_command(command)).discard()
            }
        }
    }

    fn update_media(&mut self, msg: MediaMessage) -> Task<Message> {
        match self.media_group.update(msg, self.clock) {
            media_group::Action::Run(task) => task.map(Message::Media),
            media_group::Action::FocusAppWindow(focus) => {
                self.focus_app_window(focus, Message::Media)
            }
        }
    }

    fn update_session(&mut self, msg: SessionMessage) -> Task<Message> {
        match self.session_menu.update(msg) {
            session_menu::Action::Run(task) => task.map(Message::Session),
            session_menu::Action::Performed(task) => {
                Task::batch([self.dismiss_tooltip(), task.map(Message::Session)])
            }
        }
    }

    fn update_tray(&mut self, msg: TrayMessage) -> Task<Message> {
        match self.tray_group.update(msg, self.clock) {
            tray_group::Action::None => Task::none(),
            tray_group::Action::MenuItemActivated => self.dismiss_tooltip(),
            tray_group::Action::FocusAppWindow(focus) => {
                self.focus_app_window(focus, Message::Tray)
            }
        }
    }

    fn focus_app_window<M: Send + 'static>(
        &self,
        focus: AppWindowFocus<M>,
        lift: fn(M) -> Message,
    ) -> Task<Message> {
        let placement = niri_window_placement::Placement::derive(&self.niri);
        let Some(window) = placement.window_owned_by(&focus.owner) else {
            tracing::debug!("no single window owned by {:?}", focus.owner);
            return Task::done(lift(focus.unmatched));
        };
        niri_action(niri_ipc::Action::FocusWindow { id: window })
    }

    #[must_use]
    pub fn notification_settings(&self) -> NotificationSettings {
        NotificationSettings {
            sound: self.notification_group.alert_sound_enabled(),
            toast_spatial_layout: self.overlay_settings_group.toast_spatial_layout(),
        }
    }

    #[must_use]
    pub fn tooltip_surface_style(&self) -> TooltipSurfaceStyle {
        TooltipSurfaceStyle {
            overlay_appearance: self.overlay_settings_group.overlay_appearance(),
            float_offset_px: self.panel_settings_group.float_offset_px(),
        }
    }

    fn reload_config(&self) -> Task<Message> {
        let file = self.config_file.clone();
        Task::future(blocking("config reload", move || Some(file.load()))).and_then(|loaded| {
            Task::done(Message::ConfigLoaded(
                loaded.map(Box::new).map_err(Arc::new),
            ))
        })
    }

    fn adopt_config(&mut self, config: Config) -> Task<Message> {
        let previous = mem::replace(&mut self.config, config);
        self.theme = iced::Theme::from(&PaletteColors::from(&self.config.theme.palette));
        Task::batch([
            self.notifications
                .withdraw_config_error(&self.config, self.clock.instant),
            self.adopt_fonts(&previous.theme),
            self.adopt_panel_bar(&previous.panel_bar),
            self.adopt_color_temperature_service(
                &previous.launched_programs.color_temperature_service,
            ),
        ])
    }

    fn adopt_fonts(&mut self, previous: &Theme) -> Task<Message> {
        let theme = &self.config.theme;
        if theme.font.has_same_face_as(&previous.font)
            && theme.monospace_font.family == previous.monospace_font.family
        {
            return Task::none();
        }
        FontFaces::point_generic_families_at(theme);
        font::load(self.config.theme.font.data.to_vec()).map(|loaded| {
            if let Err(e) = loaded {
                tracing::warn!("config reload: font not loaded: {e:?}");
            }
            Message::Redraw
        })
    }

    fn adopt_panel_bar(&mut self, previous: &PanelBarConfig) -> Task<Message> {
        if self.config.panel_bar == *previous {
            return Task::none();
        }
        Task::batch([self.resize_backdrops(), self.reserve()])
    }

    fn adopt_color_temperature_service(&mut self, previous: &str) -> Task<Message> {
        if self.config.launched_programs.color_temperature_service == previous {
            return Task::none();
        }
        self.color_temperature_group = ColorTemperatureGroup::new(
            self.config
                .launched_programs
                .color_temperature_service
                .clone(),
        );
        Task::none()
    }

    fn reject_config(&mut self, error: &LoadError) -> Task<Message> {
        let path = self.config_file.path();
        tracing::warn!("invalid configuration {}: {error}", path.display());
        self.notifications.show_config_error(
            path,
            error,
            &ToastContext::new(
                &self.niri,
                self.desktop_entries.get(),
                &self.config,
                self.notification_settings(),
                self.quiet_schedule_group.is_quiet(),
                self.clock,
            ),
        )
    }

    fn derive_niri_snapshot(&mut self) {
        self.niri_snapshot.load(
            niri_window_snapshot::Snapshot::derive(
                &self.niri,
                self.window_group.window_processes(),
            ),
            self.clock,
        );
    }

    fn show_niri_failure(
        &mut self,
        failure: &niri_action_requests::ActionFailure,
    ) -> Task<Message> {
        tracing::warn!("{failure}");
        self.derive_niri_snapshot();
        self.notifications.show_niri_failure(
            failure,
            &ToastContext::new(
                &self.niri,
                self.desktop_entries.get(),
                &self.config,
                self.notification_settings(),
                self.quiet_schedule_group.is_quiet(),
                self.clock,
            ),
        )
    }

    fn collapse_groups(&mut self) {
        if !self.panel_settings_group.groups_auto_collapse() {
            return;
        }
        self.panel_settings_group.collapse();
        self.sidebar_settings_group.collapse();
        self.overlay_settings_group.collapse();
        self.niri_window_group.collapse();
        self.monitor_group.collapse();
        self.color_temperature_group.collapse();
        self.dark_mode_group.collapse();
        self.voice_dictation_group.collapse();
        self.power_group.collapse();
        self.idle_timeouts_group.collapse();
        self.audio_group.collapse();
        self.wallpaper_group.collapse();
        self.caldav_group.collapse();
        self.gamepad_group.collapse();
        self.clock_group.collapse();
        self.weather_group.collapse();
        self.compute_group.collapse();
        self.memory_board_group.collapse();
        self.gpu_group.collapse();
        self.disk_group.collapse();
        self.media_group.collapse();
        self.internet_group.collapse();
        self.vpn_group.collapse();
        self.net_interfaces_group.collapse();
        self.window_group.collapse();
        self.tray_group.collapse();
        self.claude_usage_group.collapse();
        self.copilot_usage_group.collapse();
        self.openrouter_credit_group.collapse();
        self.llama_server_group.collapse();
        self.session_focus_group.collapse();
        self.login_session_group.collapse();
        self.notification_group.collapse();
        self.quiet_schedule_group.collapse();
        self.systemd_manager_group.collapse();
        self.journal_events_group.collapse();
        self.pending_upgrades_group.collapse();
    }

    fn apply_niri_events(&mut self, events: Vec<niri_ipc::Event>) -> Task<Message> {
        let mut window_change = false;
        let mut workspace_change = false;
        let mut focus_change = false;
        let mut tasks = Vec::new();
        for event in events {
            window_change |= matches!(
                event,
                niri_ipc::Event::WindowsChanged { .. }
                    | niri_ipc::Event::WindowOpenedOrChanged { .. }
                    | niri_ipc::Event::WindowClosed { .. }
            );
            workspace_change |= matches!(
                event,
                niri_ipc::Event::WorkspacesChanged { .. }
                    | niri_ipc::Event::WorkspaceActivated { .. }
            );
            focus_change |= matches!(event, niri_ipc::Event::WindowFocusChanged { .. });
            if matches!(event, niri_ipc::Event::ConfigLoaded { failed: false }) {
                tasks.push(Task::perform(
                    self.niri_config.read_workspaces(),
                    Message::NiriConfigRead,
                ));
            }
            if let Some(launch) = self.launch_tracker.observe(&event) {
                tasks.push(Task::future(sqlite_app_launches::record(launch)).discard());
            }
            let _ = self.niri.apply(event);
        }
        self.derive_niri_snapshot();
        tasks.push(self.refocus_session());
        if focus_change {
            self.collapse_groups();
        }
        if window_change {
            tasks.push(
                self.favicons
                    .track_titles(&self.niri)
                    .map(Message::FaviconsResolved),
            );
        }
        if window_change || workspace_change {
            tasks.push(self.refresh_quick_launch());
        }
        if self.notifications.is_active() {
            tasks.push(self.reflow_toasts());
        }
        Task::batch(tasks)
    }

    fn refresh_quick_launch(&mut self) -> Task<Message> {
        let Some(entries) = self.desktop_entries.arc() else {
            return Task::none();
        };
        let Some(snapshot) = self.niri_snapshot.get() else {
            return Task::none();
        };
        let now = self.clock.utc();
        let ranked = snapshot
            .active_workspaces()
            .filter_map(|active| {
                let workspace = active.name?;
                let open = snapshot.open_app_ids(active.output);
                let apps = self.launch_tracker.rank(workspace, &open, now, |app_id| {
                    entries.is_launchable(app_id)
                });
                (!apps.is_empty()).then(|| (active.output.to_owned(), apps))
            })
            .collect();
        self.window_group.set_ranked(ranked);
        self.resolve_icons()
    }

    fn resolve_icons(&mut self) -> Task<Message> {
        let snapshot = self.niri_snapshot.get();
        let mut app_keys = snapshot
            .map(desktop_entry_app_icons::app_icon_keys)
            .unwrap_or_default();
        app_keys.extend(self.window_group.icon_keys());
        let theme_keys = snapshot
            .map(desktop_entry_app_icons::theme_icon_keys)
            .unwrap_or_default();
        self.desktop_entries
            .resolve_missing(app_keys, theme_keys)
            .map(Message::IconsResolved)
    }

    fn update_panel_settings(&mut self, msg: PanelSettingsMessage) -> Task<Message> {
        match self.panel_settings_group.update(msg) {
            panel_settings_group::Action::Run(task) => task.map(Message::PanelSettings),
            panel_settings_group::Action::Reflow(task) => {
                Task::batch([self.reserve(), task.map(Message::PanelSettings)])
            }
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
            self.tooltip_surface_style(),
            self.window_assets(),
            |target| self.tooltip_for(target).unwrap_or_else(Tooltip::empty),
            self.clock,
        ) {
            return tooltip.map(Message::from);
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
            }) => Element::from(self.notifications.anchored_toasts(
                output,
                &self.config,
                &self.theme,
                self.overlay_settings_group.overlay_appearance(),
                self.bar_clearance(output),
                self.clock,
            ))
            .map(Message::Notification),
            _ => blank().into(),
        }
    }

    #[must_use]
    pub fn theme(&self, _window: Id) -> iced::Theme {
        self.theme.clone()
    }

    #[must_use]
    pub fn style(&self, _theme: &iced::Theme) -> Style {
        Style {
            background_color: Color::TRANSPARENT,
            text_color: PaletteColors::from(&self.config.theme.palette).bar_foreground,
        }
    }
}

pub(crate) fn focused_app_id(niri: &EventStreamState) -> Option<&str> {
    niri.windows
        .windows
        .values()
        .find(|window| window.is_focused)
        .and_then(|window| window.app_id.as_deref())
}

fn reheats_switch_heat(message: &Message) -> bool {
    matches!(
        message,
        Message::Niri(_) | Message::SessionFocus(_) | Message::NiriWindow(_) | Message::Clock(_)
    )
}
