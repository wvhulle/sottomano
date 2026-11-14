use sottomano_widgets::{
    hover_cards::{
        HoverTarget,
        hover_card_body::{
            HoverCardBody,
            hover_card_source_delivery::{DeliveryTiming, HoverCard},
        },
    },
    widget_groups::{
        bluetooth_adapters_group::BluetoothTarget,
        window_group::{WindowGroup, WindowTarget},
        workspaces_group::workspace_hover_card,
    },
};

use crate::desktop_shell::DesktopShell;

impl DesktopShell {
    #[must_use]
    pub fn hover_card_for(&self, target: &HoverTarget) -> Option<HoverCard> {
        match target {
            HoverTarget::PanelSettings(target) => Some(
                self.widget_groups
                    .panel_settings_group
                    .hover_card_for(*target, self.clock),
            ),
            HoverTarget::AppearancePresets(target) => Some(
                self.widget_groups
                    .appearance_presets_group
                    .hover_card_for(*target),
            ),
            HoverTarget::SidebarSettings(target) => Some(
                self.widget_groups
                    .sidebar_settings_group
                    .hover_card_for(*target, self.clock),
            ),
            HoverTarget::OverlaySettings(target) => Some(
                self.widget_groups
                    .overlay_settings_group
                    .hover_card_for(*target, self.clock),
            ),
            HoverTarget::Notification(target) => {
                Some(self.widget_groups.notification_group.hover_card_for(
                    *target,
                    self.notification_status(),
                    self.clock,
                ))
            }
            HoverTarget::AppLauncher(target) => Some(
                self.widget_groups
                    .app_launcher_group
                    .hover_card_for(target, self.clock),
            ),
            HoverTarget::GroupMenuSettings(target) => Some(
                self.widget_groups
                    .group_menus
                    .settings_hover_card(*target, self.clock),
            ),
            HoverTarget::GroupMenu(_) | HoverTarget::GroupFace(_) => Some(HoverCard::sourced(
                HoverCardBody::sections([]),
                DeliveryTiming::LivePush,
            )),
            HoverTarget::Session(target) => Some(
                self.widget_groups
                    .session_power_group
                    .hover_card_for(*target),
            ),
            HoverTarget::Tray(address) => self.widget_groups.tray_group.hover_card_for(address),
            HoverTarget::GlobalMenu(target) => self
                .widget_groups
                .global_menu_group
                .hover_card_for(target, self.clock),
            HoverTarget::Window(WindowTarget::Inspect(_)) => Some(
                self.widget_groups
                    .window_group
                    .inspect_hover_card(self.clock),
            ),
            HoverTarget::Window(WindowTarget::Usage(window)) => {
                let window_usage = self
                    .widget_groups
                    .session_focus_group
                    .window_usage_hover_card(*window, &self.config, self.clock);
                Some(
                    match WindowGroup::window_identity_hover_card(
                        &self.compositor,
                        *window,
                        self.clock,
                    ) {
                        Some(window_identity) => window_identity.with_groups(window_usage.groups),
                        None => window_usage,
                    },
                )
            }
            HoverTarget::Workspace(workspace) => workspace_hover_card::hover_card(
                &self.window_snapshot,
                &self.desktop_entries,
                self.widget_groups
                    .compositor_styling_group
                    .app_window_layout(),
                *workspace,
            ),
            HoverTarget::WorkspaceOrder | HoverTarget::WorkspaceFolding => Some(HoverCard::empty()),
            source_target => self.source_group_hover_card_for(source_target),
        }
    }

    fn bluetooth_hover_card_for(&self, target: &BluetoothTarget) -> Option<HoverCard> {
        self.widget_groups.bluetooth_adapters_group.hover_card_for(
            target,
            &|hardware_address| {
                self.widget_groups
                    .gamepad_group
                    .report_timing_for_address(hardware_address)
            },
            self.clock,
        )
    }

    #[allow(clippy::too_many_lines)]
    fn source_group_hover_card_for(&self, target: &HoverTarget) -> Option<HoverCard> {
        match target {
            HoverTarget::Compute(target) => self
                .widget_groups
                .compute_group
                .hover_card_for(*target, self.clock),
            HoverTarget::MemoryBoard(target) => self
                .widget_groups
                .memory_board_group
                .hover_card_for(target, self.clock),
            HoverTarget::Gpu(target) => self
                .widget_groups
                .gpu_group
                .hover_card_for(target, self.clock),
            HoverTarget::Storage(target) => self
                .widget_groups
                .storage_space_group
                .hover_card_for(target, self.clock),
            HoverTarget::Drive(target) => self
                .widget_groups
                .physical_drive_group
                .hover_card_for(target, self.clock),
            HoverTarget::Power(target) => self
                .widget_groups
                .power_group
                .hover_card_for(*target, self.clock),
            HoverTarget::CompositorStyling(target) => Some(
                self.widget_groups
                    .compositor_styling_group
                    .hover_card_for(*target, self.clock),
            ),
            HoverTarget::Systemd(target) => self
                .widget_groups
                .systemd_manager_group
                .hover_card_for(target, self.clock),
            HoverTarget::Journal(target) => self
                .widget_groups
                .journal_events_group
                .hover_card_for(*target, self.clock),
            HoverTarget::PendingUpgrades(target) => self
                .widget_groups
                .pending_upgrades_group
                .hover_card_for(*target, self.clock),
            HoverTarget::WifiAdapter(target) => self
                .widget_groups
                .wifi_adapters_group
                .hover_card_for(target, self.clock),
            HoverTarget::MobileModem(target) => self
                .widget_groups
                .mobile_modems_group
                .hover_card_for(target, self.clock),
            HoverTarget::EthernetPort(target) => self
                .widget_groups
                .ethernet_ports_group
                .hover_card_for(target, self.clock),
            HoverTarget::Bluetooth(target) => self.bluetooth_hover_card_for(target),
            HoverTarget::Audio(kind) => self
                .widget_groups
                .audio_group
                .hover_card_for(*kind, self.clock),
            HoverTarget::Monitor(target) => {
                self.widget_groups
                    .monitor_group
                    .hover_card_for(target, &self.outputs, self.clock)
            }
            HoverTarget::ColorTemperature(target) => self
                .widget_groups
                .color_temperature_group
                .hover_card_for(*target, self.clock),
            HoverTarget::DarkMode(target) => self
                .widget_groups
                .dark_mode_group
                .hover_card_for(*target, self.clock),
            HoverTarget::VoiceDictation(target) => self
                .widget_groups
                .voice_dictation_group
                .hover_card_for(*target, self.clock),
            HoverTarget::IdleTimeout(kind) | HoverTarget::IdleOutcome(kind) => Some(
                self.widget_groups
                    .idle_pause_groups
                    .hover_card_for(*kind, self.clock),
            ),
            HoverTarget::PauseBlocker(target) => self
                .widget_groups
                .idle_pause_groups
                .blocker_hover_card_for(target),
            HoverTarget::Reachability(target) => self
                .widget_groups
                .reachability_group
                .hover_card_for(target, self.clock),
            HoverTarget::Tailnet(target) => self
                .widget_groups
                .tailnet_group
                .hover_card_for(target, self.clock),
            HoverTarget::WireguardTunnel(target) => self
                .widget_groups
                .wireguard_tunnels_group
                .hover_card_for(target, self.clock),
            HoverTarget::NetworkManagerVpn(target) => self
                .widget_groups
                .network_manager_vpn_group
                .hover_card_for(target, self.clock),
            HoverTarget::Weather(target) => {
                self.widget_groups
                    .weather_group
                    .hover_card_for(*target, &self.config, self.clock)
            }
            HoverTarget::Wallpaper(target) => self
                .widget_groups
                .wallpaper_group
                .hover_card_for(*target, self.clock),
            HoverTarget::Caldav(target) => self.widget_groups.caldav_group.hover_card_for(
                *target,
                &self.config.widget_groups.caldav,
                self.clock,
            ),
            HoverTarget::Media(target) => self.widget_groups.media_group.hover_card_for(
                target,
                &self.desktop_entries,
                self.clock,
            ),
            HoverTarget::Gamepad(target) => self.widget_groups.gamepad_group.hover_card_for(
                *target,
                &self.config.widget_groups.gamepad,
                self.clock,
            ),
            HoverTarget::ClaudeUsage(target) => self
                .widget_groups
                .claude_usage_group
                .hover_card_for(*target, self.clock),
            HoverTarget::CopilotUsage(target) => self
                .widget_groups
                .copilot_usage_group
                .hover_card_for(*target, self.clock),
            HoverTarget::OpenRouterCredit(target) => self
                .widget_groups
                .openrouter_credit_group
                .hover_card_for(*target, self.clock),
            HoverTarget::LlamaServer(target) => self
                .widget_groups
                .llama_server_group
                .hover_card_for(target, self.clock),
            HoverTarget::PodmanEngine(target) => self
                .widget_groups
                .podman_engines_group
                .hover_card_for(target, self.clock),
            HoverTarget::VirtualMachine(target) => self
                .widget_groups
                .virtual_machines_group
                .hover_card_for(target, self.clock),
            HoverTarget::NspawnContainer(target) => self
                .widget_groups
                .nspawn_containers_group
                .hover_card_for(target, self.clock),
            HoverTarget::MicrovmGuest(target) => self
                .widget_groups
                .microvm_guests_group
                .hover_card_for(target, self.clock),
            HoverTarget::IncusInstance(target) => self
                .widget_groups
                .incus_instances_group
                .hover_card_for(target, self.clock),
            HoverTarget::LxcContainer(target) => self
                .widget_groups
                .lxc_containers_group
                .hover_card_for(target, self.clock),
            HoverTarget::FlatpakSandbox(target) => self
                .widget_groups
                .flatpak_sandboxes_group
                .hover_card_for(target, self.clock),
            HoverTarget::WaydroidContainer(target) => self
                .widget_groups
                .waydroid_container_group
                .hover_card_for(target, self.clock),
            HoverTarget::VirtualBoxMachine(target) => self
                .widget_groups
                .virtualbox_machines_group
                .hover_card_for(target, self.clock),
            HoverTarget::VmwareMachine(target) => self
                .widget_groups
                .vmware_machines_group
                .hover_card_for(target, self.clock),
            HoverTarget::ApptainerContainer(target) => self
                .widget_groups
                .apptainer_containers_group
                .hover_card_for(target, self.clock),
            HoverTarget::DockerEngine(target) => self
                .widget_groups
                .docker_engines_group
                .hover_card_for(target, self.clock),
            HoverTarget::CgroupMemoryLimit(target) => self
                .widget_groups
                .cgroup_memory_limits_group
                .hover_card_for(target),
            HoverTarget::CiRuns(target) => self
                .widget_groups
                .ci_runs_group
                .hover_card_for(target, self.clock),
            HoverTarget::SessionFocus(target) => {
                Some(self.widget_groups.session_focus_group.hover_card_for(
                    *target,
                    &self.config,
                    self.clock,
                ))
            }
            HoverTarget::LoginSession(target) => self
                .widget_groups
                .login_session_group
                .hover_card_for(*target, self.clock),
            HoverTarget::Clock(target) => self
                .widget_groups
                .clock_group
                .hover_card_for(target, self.clock),
            HoverTarget::QuietSchedule(target) => self
                .widget_groups
                .quiet_schedule_group
                .hover_card_for(*target, self.config.calendar.locale, self.clock),
            HoverTarget::PanelSettings(_)
            | HoverTarget::SidebarSettings(_)
            | HoverTarget::AppearancePresets(_)
            | HoverTarget::OverlaySettings(_)
            | HoverTarget::Notification(_)
            | HoverTarget::AppLauncher(_)
            | HoverTarget::GroupMenu(_)
            | HoverTarget::GroupFace(_)
            | HoverTarget::GroupMenuSettings(_)
            | HoverTarget::Session(_)
            | HoverTarget::Tray(_)
            | HoverTarget::GlobalMenu(_)
            | HoverTarget::Window(_)
            | HoverTarget::Workspace(_)
            | HoverTarget::WorkspaceOrder
            | HoverTarget::WorkspaceFolding => None,
        }
    }
}
