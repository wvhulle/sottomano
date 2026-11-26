mod group_actions;
mod group_appearance_settings;
mod group_delivery_routing;
mod group_hover_cards;
mod group_routing;
mod group_subscriptions;

use iced::Task;
use sottomano_compositor_model::compositor_window_state::CompositorWindowState;
use sottomano_desktop_sinks::compositor_rule_files::CompositorRulesFile;
use sottomano_toml_config::{Config, SecretPaths, secrets::StoredSecret};
use sottomano_widgets::{
    cascading_menus::{app_launcher_menu::AppLauncherMenu, window_menu_bar::WindowMenuBar},
    clock::Clock,
    group_menus::group_menu_state::GroupMenuState,
    output_windows::OutputWindows,
    panel_menus::{
        agenda_menu::{
            caldav_group::CaldavGroup, clock_group::ClockGroup, location_group::LocationGroup,
            notification_group::NotificationGroup, quiet_schedule_group::QuietScheduleGroup,
            session_focus_group::SessionFocusGroup, weather_group::WeatherGroup,
        },
        appearance_menu::{
            compositor_styling_group::CompositorStylingGroup,
            overlay_settings_group::OverlaySettingsGroup,
            panel_settings_group::{GroupCollapsing, PanelSettingsGroup},
            sidebar_settings_group::SidebarSettingsGroup,
        },
        behaviour_menu::appearance_presets_group::AppearancePresetsGroup,
        display_menu::{
            color_temperature_group::ColorTemperatureGroup, dark_mode_group::DarkModeGroup,
            monitor_group::MonitorGroup, wallpaper_group::WallpaperGroup,
        },
        gaming_menu::{gamepad_group::GamepadGroup, steam_client_groups::SteamClientGroups},
        hardware_menu::{
            compute_group::ComputeGroup, gpu_group::GpuGroup, memory_board_group::MemoryBoardGroup,
            physical_drive_group::PhysicalDriveGroup, power_group::PowerGroup,
            storage_space_group::StorageSpaceGroup,
        },
        media_menu::{
            audio_group::AudioGroup, media_group::MediaGroup,
            voice_dictation_group::VoiceDictationGroup,
        },
        network_menu::{
            bluetooth_adapters_group::BluetoothAdaptersGroup,
            ethernet_ports_group::EthernetPortsGroup, mobile_modems_group::MobileModemsGroup,
            network_manager_vpn_group::NetworkManagerVpnGroup,
            reachability_group::ReachabilityGroup, tailnet_group::TailnetGroup,
            wifi_adapters_group::WifiAdaptersGroup, wireguard_tunnels_group::WireguardTunnelsGroup,
        },
        services_menu::{
            ci_runs_group::CiRunsGroup, claude_usage_group::ClaudeUsageGroup,
            copilot_usage_group::CopilotUsageGroup, journal_events_group::JournalEventsGroup,
            llama_server_group::LlamaServerGroup, openrouter_credit_group::OpenRouterCreditGroup,
            pending_upgrades_group::PendingUpgradesGroup,
            systemd_manager_group::SystemdManagerGroup,
        },
        session_menu::{
            idle_pause_groups::IdlePauseGroups, login_session_group::LoginSessionGroup,
            tray_group::TrayGroup,
        },
        session_power_menu::session_power_group::SessionPowerGroup,
        virtualization_menu::{
            apptainer_containers_group::ApptainerContainersGroup,
            cgroup_memory_limits_group::CgroupMemoryLimitsGroup,
            docker_engines_group::DockerEnginesGroup,
            flatpak_sandboxes_group::FlatpakSandboxesGroup,
            incus_instances_group::IncusInstancesGroup, lxc_containers_group::LxcContainersGroup,
            microvm_guests_group::MicrovmGuestsGroup,
            nspawn_containers_group::NspawnContainersGroup,
            podman_engines_group::PodmanEnginesGroup, virtual_machines_group::VirtualMachinesGroup,
            virtualbox_machines_group::VirtualBoxMachinesGroup,
            vmware_machines_group::VmwareMachinesGroup,
            waydroid_container_group::WaydroidContainerGroup,
        },
    },
};

use crate::shell_message::Message;

pub struct ShellIndicatorGroups {
    pub weather_group: WeatherGroup,
    pub location_group: LocationGroup,
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
    pub compositor_styling_group: CompositorStylingGroup,
    pub clock_group: ClockGroup,
    pub notification_group: NotificationGroup,
    pub quiet_schedule_group: QuietScheduleGroup,
    pub session_power_group: SessionPowerGroup,
    pub systemd_manager_group: SystemdManagerGroup,
    pub journal_events_group: JournalEventsGroup,
    pub pending_upgrades_group: PendingUpgradesGroup,
    pub reachability_group: ReachabilityGroup,
    pub tailnet_group: TailnetGroup,
    pub wireguard_tunnels_group: WireguardTunnelsGroup,
    pub network_manager_vpn_group: NetworkManagerVpnGroup,
    pub wifi_adapters_group: WifiAdaptersGroup,
    pub mobile_modems_group: MobileModemsGroup,
    pub steam_client_groups: SteamClientGroups,
    pub ethernet_ports_group: EthernetPortsGroup,
    pub bluetooth_adapters_group: BluetoothAdaptersGroup,
    pub monitor_group: MonitorGroup,
    pub color_temperature_group: ColorTemperatureGroup,
    pub dark_mode_group: DarkModeGroup,
    pub voice_dictation_group: VoiceDictationGroup,
    pub idle_pause_groups: IdlePauseGroups,
    pub group_menus: GroupMenuState,
    pub media_group: MediaGroup,
    pub gamepad_group: GamepadGroup,
    pub audio_group: AudioGroup,
    pub claude_usage_group: ClaudeUsageGroup,
    pub copilot_usage_group: CopilotUsageGroup,
    pub openrouter_credit_group: OpenRouterCreditGroup,
    pub llama_server_group: LlamaServerGroup,
    pub podman_engines_group: PodmanEnginesGroup,
    pub virtual_machines_group: VirtualMachinesGroup,
    pub nspawn_containers_group: NspawnContainersGroup,
    pub cgroup_memory_limits_group: CgroupMemoryLimitsGroup,
    pub docker_engines_group: DockerEnginesGroup,
    pub apptainer_containers_group: ApptainerContainersGroup,
    pub vmware_machines_group: VmwareMachinesGroup,
    pub virtualbox_machines_group: VirtualBoxMachinesGroup,
    pub waydroid_container_group: WaydroidContainerGroup,
    pub flatpak_sandboxes_group: FlatpakSandboxesGroup,
    pub lxc_containers_group: LxcContainersGroup,
    pub incus_instances_group: IncusInstancesGroup,
    pub microvm_guests_group: MicrovmGuestsGroup,
    pub ci_runs_group: CiRunsGroup,
    pub session_focus_group: SessionFocusGroup,
    pub login_session_group: LoginSessionGroup,
    pub tray_group: TrayGroup,
    pub window_menu_bar: WindowMenuBar,
    pub output_windows: OutputWindows,
    pub app_launcher_menu: AppLauncherMenu,
}

impl ShellIndicatorGroups {
    pub fn new(
        config: &Config,
        secrets: SecretPaths,
        compositor_rules: CompositorRulesFile,
        clock: Clock,
    ) -> Self {
        Self {
            weather_group: WeatherGroup::default(),
            location_group: LocationGroup::default(),
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
            appearance_presets_group: AppearancePresetsGroup::default(),
            overlay_settings_group: OverlaySettingsGroup::default(),
            compositor_styling_group: CompositorStylingGroup::new(compositor_rules),
            clock_group: ClockGroup::default(),
            notification_group: NotificationGroup::default(),
            quiet_schedule_group: QuietScheduleGroup::default(),
            session_power_group: SessionPowerGroup::default(),
            systemd_manager_group: SystemdManagerGroup::default(),
            journal_events_group: JournalEventsGroup::default(),
            pending_upgrades_group: PendingUpgradesGroup::default(),
            reachability_group: ReachabilityGroup::default(),
            tailnet_group: match secrets.tailscale_api_token {
                StoredSecret::Present(token_path) => TailnetGroup::with_policy_file(token_path),
                StoredSecret::Absent => TailnetGroup::without_policy_file(),
            },
            wireguard_tunnels_group: WireguardTunnelsGroup::default(),
            network_manager_vpn_group: NetworkManagerVpnGroup::default(),
            wifi_adapters_group: WifiAdaptersGroup::default(),
            mobile_modems_group: MobileModemsGroup::default(),
            steam_client_groups: SteamClientGroups::default(),
            ethernet_ports_group: EthernetPortsGroup::default(),
            bluetooth_adapters_group: BluetoothAdaptersGroup::default(),
            monitor_group: MonitorGroup::default(),
            color_temperature_group: ColorTemperatureGroup::new(
                config.launched_programs.color_temperature_service.clone(),
            ),
            dark_mode_group: DarkModeGroup::new(clock),
            voice_dictation_group: VoiceDictationGroup::new(),
            idle_pause_groups: IdlePauseGroups::default(),
            group_menus: GroupMenuState::default(),
            media_group: MediaGroup::default(),
            gamepad_group: GamepadGroup::default(),
            audio_group: AudioGroup::default(),
            claude_usage_group: ClaudeUsageGroup::new(secrets.claude_code_oauth),
            copilot_usage_group: CopilotUsageGroup::new(secrets.gh_auth_token),
            openrouter_credit_group: OpenRouterCreditGroup::new(secrets.openrouter_management),
            llama_server_group: LlamaServerGroup::default(),
            podman_engines_group: PodmanEnginesGroup::default(),
            virtual_machines_group: VirtualMachinesGroup::default(),
            nspawn_containers_group: NspawnContainersGroup::default(),
            cgroup_memory_limits_group: CgroupMemoryLimitsGroup::default(),
            docker_engines_group: DockerEnginesGroup::default(),
            apptainer_containers_group: ApptainerContainersGroup::default(),
            vmware_machines_group: VmwareMachinesGroup::default(),
            virtualbox_machines_group: VirtualBoxMachinesGroup::default(),
            waydroid_container_group: WaydroidContainerGroup::default(),
            flatpak_sandboxes_group: FlatpakSandboxesGroup::default(),
            lxc_containers_group: LxcContainersGroup::default(),
            incus_instances_group: IncusInstancesGroup::default(),
            microvm_guests_group: MicrovmGuestsGroup::default(),
            ci_runs_group: CiRunsGroup::default(),
            session_focus_group: SessionFocusGroup::started_at(
                &config.panel_menus.agenda.session_focus,
                clock.utc(),
            ),
            login_session_group: LoginSessionGroup::default(),
            tray_group: TrayGroup::default(),
            window_menu_bar: WindowMenuBar::default(),
            output_windows: OutputWindows::default(),
            app_launcher_menu: AppLauncherMenu::default(),
        }
    }

    pub fn load(&self, clock: Clock) -> Task<Message> {
        Task::batch([
            SessionFocusGroup::load(clock).map(Message::SessionFocus),
            SessionPowerGroup::load().map(Message::Session),
            IdlePauseGroups::load().map(Message::IdleTimeouts),
            GroupMenuState::load().map(Message::GroupMenuState),
            AppLauncherMenu::load().map(Message::AppLauncher),
            PanelSettingsGroup::load().map(Message::PanelSettings),
            SidebarSettingsGroup::load().map(Message::SidebarSettings),
            OverlaySettingsGroup::load().map(Message::OverlaySettings),
            WindowMenuBar::load().map(Message::WindowMenuBar),
            NotificationGroup::load().map(Message::NotificationGroup),
            QuietScheduleGroup::load().map(Message::QuietSchedule),
            DarkModeGroup::load().map(Message::DarkMode),
            GamepadGroup::load().map(Message::Gamepad),
            self.compositor_styling_group
                .load()
                .map(Message::CompositorStyling),
        ])
    }

    pub fn reheat_switch_heat(
        &mut self,
        config: &Config,
        compositor: &CompositorWindowState,
        clock: Clock,
    ) -> Task<Message> {
        let session_focus_group = &self.session_focus_group;
        self.compositor_styling_group
            .reheat(config, || {
                session_focus_group.heated_windows(compositor, config, clock)
            })
            .map(Message::CompositorStyling)
    }

    pub fn adopt_config(&mut self, previous: &Config, config: &Config) {
        let service = &config.launched_programs.color_temperature_service;
        if *service != previous.launched_programs.color_temperature_service {
            self.color_temperature_group = ColorTemperatureGroup::new(service.clone());
        }
    }

    pub fn collapse_when_auto_collapsing(&mut self) {
        if self.panel_settings_group.group_collapsing == GroupCollapsing::StayExpanded {
            return;
        }
        self.panel_settings_group.collapse();
        self.sidebar_settings_group.collapse();
        self.overlay_settings_group.collapse();
        self.compositor_styling_group.collapse();
        self.monitor_group.collapse();
        self.color_temperature_group.collapse();
        self.dark_mode_group.collapse();
        self.voice_dictation_group.collapse();
        self.power_group.collapse();
        self.idle_pause_groups.collapse();
        self.audio_group.collapse();
        self.wallpaper_group.collapse();
        self.appearance_presets_group.collapse();
        self.caldav_group.collapse();
        self.gamepad_group.collapse();
        self.clock_group.collapse();
        self.weather_group.collapse();
        self.location_group.collapse();
        self.compute_group.collapse();
        self.memory_board_group.collapse();
        self.gpu_group.collapse();
        self.storage_space_group.collapse();
        self.physical_drive_group.collapse();
        self.media_group.collapse();
        self.reachability_group.collapse();
        self.tailnet_group.collapse();
        self.wireguard_tunnels_group.collapse();
        self.network_manager_vpn_group.collapse();
        self.wifi_adapters_group.collapse();
        self.mobile_modems_group.collapse();
        self.steam_client_groups.collapse();
        self.ethernet_ports_group.collapse();
        self.bluetooth_adapters_group.collapse();
        self.tray_group.collapse();
        self.window_menu_bar.collapse();
        self.claude_usage_group.collapse();
        self.copilot_usage_group.collapse();
        self.openrouter_credit_group.collapse();
        self.llama_server_group.collapse();
        self.podman_engines_group.collapse();
        self.virtual_machines_group.collapse();
        self.nspawn_containers_group.collapse();
        self.cgroup_memory_limits_group.collapse();
        self.docker_engines_group.collapse();
        self.apptainer_containers_group.collapse();
        self.vmware_machines_group.collapse();
        self.virtualbox_machines_group.collapse();
        self.waydroid_container_group.collapse();
        self.flatpak_sandboxes_group.collapse();
        self.lxc_containers_group.collapse();
        self.incus_instances_group.collapse();
        self.microvm_guests_group.collapse();
        self.ci_runs_group.collapse();
        self.session_focus_group.collapse();
        self.login_session_group.collapse();
        self.notification_group.collapse();
        self.quiet_schedule_group.collapse();
        self.systemd_manager_group.collapse();
        self.journal_events_group.collapse();
        self.pending_upgrades_group.collapse();
        self.group_menus.collapse_settings();
    }
}
