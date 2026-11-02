use iced::Task;
use niri_ipc::state::EventStreamState;
use sottomano_config::{Config, SecretPaths};
use sottomano_sources::desktop_windows_apps::desktop_entry_app_icons::DesktopEntries;
use sottomano_widgets::{
    clock::Clock,
    hover_tooltips::tooltip_surfaces::TooltipSurfaceStyle,
    notification_toasts::{NotificationSettings, ToastContext},
    widget_groups::{
        appearance_presets_group::AppearancePresetsGroup,
        audio_group::AudioGroup,
        bluetooth_adapters_group::BluetoothAdaptersGroup,
        caldav_group::CaldavGroup,
        claude_usage_group::ClaudeUsageGroup,
        clock_group::ClockGroup,
        color_temperature_group::ColorTemperatureGroup,
        compute_group::{ComputeGroup, ComputeMessage},
        copilot_usage_group::CopilotUsageGroup,
        dark_mode_group::{DarkModeGroup, DarkModeMessage},
        ethernet_ports_group::EthernetPortsGroup,
        gamepad_group::GamepadGroup,
        gpu_group::GpuGroup,
        group_menus::GroupMenuState,
        idle_timeouts_group::IdleTimeoutsGroup,
        journal_events_group::JournalEventsGroup,
        llama_server_group::LlamaServerGroup,
        login_session_group::LoginSessionGroup,
        media_group::MediaGroup,
        memory_board_group::{MemoryBoardGroup, MemoryBoardMessage},
        mobile_modems_group::MobileModemsGroup,
        monitor_group::MonitorGroup,
        niri_window_group::{NiriWindowGroup, niri_kdl_rules::NiriRulesFile},
        notification_group::NotificationGroup,
        openrouter_credit_group::OpenRouterCreditGroup,
        overlay_settings_group::{OverlaySettingsGroup, OverlaySettingsMessage},
        panel_settings_group::{PanelSettingsGroup, PanelSettingsMessage},
        pending_upgrades_group::PendingUpgradesGroup,
        physical_drive_group::PhysicalDriveGroup,
        power_group::PowerGroup,
        quiet_schedule_group::QuietScheduleGroup,
        reachability_group::ReachabilityGroup,
        session_focus_group::SessionFocusGroup,
        session_power_group::SessionPowerGroup,
        sidebar_settings_group::{SidebarSettingsGroup, SidebarSettingsMessage},
        storage_space_group::StorageSpaceGroup,
        systemd_manager_group::SystemdManagerGroup,
        tray_group::TrayGroup,
        voice_dictation_group::VoiceDictationGroup,
        vpn_group::VpnGroup,
        wallpaper_group::WallpaperGroup,
        weather_group::WeatherGroup,
        wifi_adapters_group::WifiAdaptersGroup,
        window_group::WindowGroup,
    },
    widgets::{
        appearance_presets::{AppearancePreset, ShellAppearance},
        layer_appearance_controls::AppearanceMessage,
    },
};

use crate::shell_message::Message;

pub struct ShellWidgetGroups {
    pub weather_group: WeatherGroup,
    pub wallpaper_group: WallpaperGroup,
    pub caldav_group: CaldavGroup,
    pub power_group: PowerGroup,
    pub compute_group: ComputeGroup,
    pub memory_board_group: MemoryBoardGroup,
    pub gpu_group: GpuGroup,
    pub storage_space_group: StorageSpaceGroup,
    pub physical_drive_group: PhysicalDriveGroup,
    pub panel_settings_group: PanelSettingsGroup,
    pub sidebar_settings_group: SidebarSettingsGroup,
    pub appearance_presets_group: AppearancePresetsGroup,
    pub overlay_settings_group: OverlaySettingsGroup,
    pub niri_window_group: NiriWindowGroup,
    pub clock_group: ClockGroup,
    pub notification_group: NotificationGroup,
    pub quiet_schedule_group: QuietScheduleGroup,
    pub session_power_group: SessionPowerGroup,
    pub systemd_manager_group: SystemdManagerGroup,
    pub journal_events_group: JournalEventsGroup,
    pub pending_upgrades_group: PendingUpgradesGroup,
    pub reachability_group: ReachabilityGroup,
    pub vpn_group: VpnGroup,
    pub wifi_adapters_group: WifiAdaptersGroup,
    pub mobile_modems_group: MobileModemsGroup,
    pub ethernet_ports_group: EthernetPortsGroup,
    pub bluetooth_adapters_group: BluetoothAdaptersGroup,
    pub monitor_group: MonitorGroup,
    pub color_temperature_group: ColorTemperatureGroup,
    pub dark_mode_group: DarkModeGroup,
    pub voice_dictation_group: VoiceDictationGroup,
    pub idle_timeouts_group: IdleTimeoutsGroup,
    pub group_menus: GroupMenuState,
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
}

impl ShellWidgetGroups {
    pub fn new(
        config: &Config,
        secrets: SecretPaths,
        niri_rules: NiriRulesFile,
        clock: Clock,
    ) -> Self {
        Self {
            weather_group: WeatherGroup::default(),
            wallpaper_group: WallpaperGroup::new(&secrets),
            caldav_group: CaldavGroup::new(&secrets),
            power_group: PowerGroup::default(),
            compute_group: ComputeGroup::default(),
            memory_board_group: MemoryBoardGroup::default(),
            gpu_group: GpuGroup::default(),
            storage_space_group: StorageSpaceGroup::default(),
            physical_drive_group: PhysicalDriveGroup::default(),
            panel_settings_group: PanelSettingsGroup::default(),
            sidebar_settings_group: SidebarSettingsGroup::default(),
            appearance_presets_group: AppearancePresetsGroup,
            overlay_settings_group: OverlaySettingsGroup::default(),
            niri_window_group: NiriWindowGroup::new(niri_rules),
            clock_group: ClockGroup::default(),
            notification_group: NotificationGroup::default(),
            quiet_schedule_group: QuietScheduleGroup::default(),
            session_power_group: SessionPowerGroup::default(),
            systemd_manager_group: SystemdManagerGroup::default(),
            journal_events_group: JournalEventsGroup::default(),
            pending_upgrades_group: PendingUpgradesGroup::default(),
            reachability_group: ReachabilityGroup::default(),
            vpn_group: VpnGroup::default(),
            wifi_adapters_group: WifiAdaptersGroup::default(),
            mobile_modems_group: MobileModemsGroup::default(),
            ethernet_ports_group: EthernetPortsGroup::default(),
            bluetooth_adapters_group: BluetoothAdaptersGroup::default(),
            monitor_group: MonitorGroup::default(),
            color_temperature_group: ColorTemperatureGroup::new(
                config.launched_programs.color_temperature_service.clone(),
            ),
            dark_mode_group: DarkModeGroup::new(clock),
            voice_dictation_group: VoiceDictationGroup::new(),
            idle_timeouts_group: IdleTimeoutsGroup::default(),
            group_menus: GroupMenuState::default(),
            media_group: MediaGroup::default(),
            gamepad_group: GamepadGroup::default(),
            audio_group: AudioGroup::default(),
            claude_usage_group: ClaudeUsageGroup::new(secrets.claude_code_oauth),
            copilot_usage_group: CopilotUsageGroup::new(secrets.gh_auth_token),
            openrouter_credit_group: OpenRouterCreditGroup::new(
                secrets.openrouter_management,
                secrets.artificial_analysis_api,
            ),
            llama_server_group: LlamaServerGroup::default(),
            session_focus_group: SessionFocusGroup::started_at(
                &config.widget_groups.focus_time,
                clock.utc(),
            ),
            login_session_group: LoginSessionGroup::default(),
            tray_group: TrayGroup::default(),
            window_group: WindowGroup::default(),
        }
    }

    pub fn load(&self, clock: Clock) -> Task<Message> {
        Task::batch([
            SessionFocusGroup::load(clock).map(Message::SessionFocus),
            SessionPowerGroup::load().map(Message::Session),
            IdleTimeoutsGroup::load().map(Message::IdleTimeouts),
            GroupMenuState::load().map(Message::GroupMenuState),
            PanelSettingsGroup::load().map(Message::PanelSettings),
            SidebarSettingsGroup::load().map(Message::SidebarSettings),
            OverlaySettingsGroup::load().map(Message::OverlaySettings),
            NotificationGroup::load().map(Message::NotificationGroup),
            QuietScheduleGroup::load().map(Message::QuietSchedule),
            DarkModeGroup::load().map(Message::DarkMode),
            self.niri_window_group.load().map(Message::NiriWindow),
        ])
    }

    #[allow(clippy::too_many_lines)]
    pub fn route(&mut self, message: Message, config: &Config, clock: Clock) -> Task<Message> {
        match message {
            Message::Weather(msg) => self.weather_group.update(msg, clock).map(Message::Weather),
            Message::Wallpaper(msg) => self
                .wallpaper_group
                .update(msg, clock)
                .map(Message::Wallpaper),
            Message::Caldav(msg) => self.caldav_group.update(msg, clock).map(Message::Caldav),
            Message::Power(msg) => self.power_group.update(msg, clock).map(Message::Power),
            Message::Compute(msg) => self.compute_group.update(msg, clock).map(Message::Compute),
            Message::MemoryBoard(msg) => self
                .memory_board_group
                .update(msg, clock)
                .map(Message::MemoryBoard),
            Message::HostHardware(delivery) => Task::batch([
                self.compute_group
                    .update(ComputeMessage::HostHardware(delivery.clone()), clock)
                    .map(Message::Compute),
                self.memory_board_group
                    .update(MemoryBoardMessage::Hardware(delivery), clock)
                    .map(Message::MemoryBoard),
            ]),
            Message::Gpu(msg) => self.gpu_group.update(msg, clock).map(Message::Gpu),
            Message::Storage(msg) => self
                .storage_space_group
                .update(msg, clock)
                .map(Message::Storage),
            Message::Drive(msg) => self
                .physical_drive_group
                .update(msg, clock)
                .map(Message::Drive),
            Message::SidebarSettings(msg) => self
                .sidebar_settings_group
                .update(msg)
                .map(Message::SidebarSettings),
            Message::NiriWindow(msg) => self.niri_window_group.update(msg).map(Message::NiriWindow),
            Message::Clock(msg) => self.clock_group.update(msg, clock).map(Message::Clock),
            Message::SystemdManager(msg) => self
                .systemd_manager_group
                .update(msg, clock)
                .map(Message::SystemdManager),
            Message::JournalEvents(msg) => self
                .journal_events_group
                .update(msg, clock)
                .map(Message::JournalEvents),
            Message::PendingUpgrades(msg) => self
                .pending_upgrades_group
                .update(msg, clock)
                .map(Message::PendingUpgrades),
            Message::WifiAdapters(msg) => self
                .wifi_adapters_group
                .update(msg, clock)
                .map(Message::WifiAdapters),
            Message::MobileModems(msg) => self
                .mobile_modems_group
                .update(msg, clock)
                .map(Message::MobileModems),
            Message::EthernetPorts(msg) => self
                .ethernet_ports_group
                .update(msg, clock)
                .map(Message::EthernetPorts),
            Message::BluetoothAdapters(msg) => self
                .bluetooth_adapters_group
                .update(msg, clock)
                .map(Message::BluetoothAdapters),
            Message::Reachability(msg) => self
                .reachability_group
                .update(msg, &config.widget_groups.reachability, clock)
                .map(Message::Reachability),
            Message::Vpn(msg) => self.vpn_group.update(msg, clock).map(Message::Vpn),
            Message::Monitor(msg) => self.monitor_group.update(msg, clock).map(Message::Monitor),
            Message::ColorTemperature(msg) => self
                .color_temperature_group
                .update(msg, clock)
                .map(Message::ColorTemperature),
            Message::DarkMode(msg) => self
                .dark_mode_group
                .update(msg, clock)
                .map(Message::DarkMode),
            Message::VoiceDictation(msg) => self
                .voice_dictation_group
                .update(msg, clock)
                .map(Message::VoiceDictation),
            Message::Audio(msg) => self.audio_group.update(msg, clock).map(Message::Audio),
            Message::ClaudeUsage(msg) => self
                .claude_usage_group
                .update(msg, clock)
                .map(Message::ClaudeUsage),
            Message::CopilotUsage(msg) => self
                .copilot_usage_group
                .update(msg, clock)
                .map(Message::CopilotUsage),
            Message::OpenRouterCredit(msg) => self
                .openrouter_credit_group
                .update(msg, clock)
                .map(Message::OpenRouterCredit),
            Message::LlamaServer(msg) => self
                .llama_server_group
                .update(msg, config, clock)
                .map(Message::LlamaServer),
            Message::SessionFocus(msg) => self
                .session_focus_group
                .update(msg, &config.widget_groups.focus_time, clock)
                .map(Message::SessionFocus),
            Message::LoginSession(msg) => self
                .login_session_group
                .update(msg, clock)
                .map(Message::LoginSession),
            unrouted => {
                tracing::warn!(?unrouted, "not a widget group message");
                Task::none()
            }
        }
    }

    pub fn reheat_switch_heat(
        &mut self,
        config: &Config,
        niri: &EventStreamState,
        clock: Clock,
    ) -> Task<Message> {
        let session_focus_group = &self.session_focus_group;
        self.niri_window_group
            .reheat(config, || {
                session_focus_group.heated_windows(niri, config, clock)
            })
            .map(Message::NiriWindow)
    }

    #[must_use]
    pub fn shell_appearance(&self) -> ShellAppearance {
        ShellAppearance {
            scheme: self.dark_mode_group.scheme(),
            panels: self.panel_settings_group.panel_appearance(),
            sidebar: self.sidebar_settings_group.sidebar_appearance(),
            overlays: self.overlay_settings_group.overlay_appearance(),
        }
    }

    pub fn apply_appearance_preset(&self, preset: AppearancePreset) -> Task<Message> {
        let current = self.shell_appearance();
        let next = preset.applied_to(current);
        let scheme_switch = (next.scheme != current.scheme).then(|| {
            Task::done(Message::DarkMode(DarkModeMessage::SwitchScheme(
                next.scheme,
            )))
        });
        Task::batch(
            [
                Task::done(Message::PanelSettings(PanelSettingsMessage::Appearance(
                    AppearanceMessage::Adopted(next.panels),
                ))),
                Task::done(Message::SidebarSettings(
                    SidebarSettingsMessage::Appearance(AppearanceMessage::Adopted(next.sidebar)),
                )),
                Task::done(Message::OverlaySettings(
                    OverlaySettingsMessage::Appearance(AppearanceMessage::Adopted(next.overlays)),
                )),
            ]
            .into_iter()
            .chain(scheme_switch),
        )
    }

    pub fn adopt_config(&mut self, previous: &Config, config: &Config) {
        let service = &config.launched_programs.color_temperature_service;
        if *service != previous.launched_programs.color_temperature_service {
            self.color_temperature_group = ColorTemperatureGroup::new(service.clone());
        }
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

    #[must_use]
    pub fn toast_context<'shell>(
        &self,
        niri: &EventStreamState,
        entries: Option<&'shell DesktopEntries>,
        config: &'shell Config,
        clock: Clock,
    ) -> ToastContext<'shell> {
        ToastContext::new(
            niri,
            entries,
            config,
            self.notification_settings(),
            self.quiet_schedule_group.is_quiet(),
            clock,
        )
    }

    pub fn collapse_when_auto_collapsing(&mut self) {
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
        self.storage_space_group.collapse();
        self.physical_drive_group.collapse();
        self.media_group.collapse();
        self.reachability_group.collapse();
        self.vpn_group.collapse();
        self.wifi_adapters_group.collapse();
        self.mobile_modems_group.collapse();
        self.ethernet_ports_group.collapse();
        self.bluetooth_adapters_group.collapse();
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
}
