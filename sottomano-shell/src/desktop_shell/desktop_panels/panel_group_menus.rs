use std::iter::once;

use sottomano_desktop_sources::desktop_windows_apps::window_edge_placement::Placement;
use sottomano_widgets::{
    group_menus::{group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView},
    widget_groups::{
        fixed_panel_group::FixedPanelGroup, notification_group::NotificationStatus,
        widget_group_content::WidgetGroupContent,
    },
};

use crate::{
    desktop_shell::{
        DesktopShell,
        desktop_panels::panel_bar_composition::{FixedPanelEntry, StandaloneWidgetGroup},
    },
    shell_message::Message,
};

impl DesktopShell {
    pub(in crate::desktop_shell) fn group_menu_of(
        &self,
        topic: GroupMenuTopic,
        output: &str,
    ) -> GroupMenuView<'_, Message> {
        match topic {
            GroupMenuTopic::Applications => self.group_menu(
                GroupMenuTopic::Applications,
                self.widget_groups
                    .app_launcher_group
                    .group_contents(&self.desktop_entries, &self.config, output)
                    .into_iter()
                    .map(|group| group.map(Message::AppLauncher)),
            ),
            GroupMenuTopic::ApplicationMenu => self.group_menu(
                GroupMenuTopic::ApplicationMenu,
                self.widget_groups
                    .global_menu_group
                    .group_contents()
                    .into_iter()
                    .map(|group| group.map(Message::GlobalMenu)),
            ),
            GroupMenuTopic::Hardware => self.hardware_group_menu(),
            GroupMenuTopic::Network => self.network_group_menu(),
            GroupMenuTopic::Agenda => self.agenda_group_menu(),
            GroupMenuTopic::Media => self.media_group_menu(),
            GroupMenuTopic::Display => self.display_group_menu(),
            GroupMenuTopic::Virtualization => self.virtualization_group_menu(),
            GroupMenuTopic::Services => self.services_group_menu(),
            GroupMenuTopic::Session => self.session_group_menu(),
            GroupMenuTopic::Appearance => self.group_menu(
                GroupMenuTopic::Appearance,
                self.settings_groups()
                    .map(WidgetGroupContent::only_aligned_items),
            ),
            GroupMenuTopic::Behaviour => self.group_menu(
                GroupMenuTopic::Behaviour,
                self.settings_groups()
                    .map(WidgetGroupContent::only_unaligned_items)
                    .into_iter()
                    .chain(once(
                        self.widget_groups
                            .appearance_presets_group
                            .group_content(&self.widget_groups.shell_appearance())
                            .map(Message::AppearancePresets),
                    )),
            ),
        }
    }

    pub(in crate::desktop_shell) fn fixed_panel_group(
        &self,
        entry: FixedPanelEntry,
    ) -> FixedPanelGroup<'_, Message> {
        match entry {
            FixedPanelEntry::SessionPower => self
                .widget_groups
                .session_power_group
                .group_content()
                .map(Message::Session),
        }
    }

    pub(in crate::desktop_shell) fn standalone_group(
        &self,
        group: StandaloneWidgetGroup,
    ) -> WidgetGroupContent<'_, Message> {
        let config = &self.config;
        let window_placement = Placement::derive(&self.compositor);
        match group {
            StandaloneWidgetGroup::Clock => self
                .widget_groups
                .clock_group
                .group_content()
                .map(Message::Clock),
            StandaloneWidgetGroup::Notifications => self
                .widget_groups
                .notification_group
                .group_content(self.notification_status())
                .map(Message::NotificationGroup),
            StandaloneWidgetGroup::Tray => self
                .widget_groups
                .tray_group
                .group_content(config, &|owner| {
                    window_placement.window_owned_by(owner).is_some()
                })
                .map(Message::Tray),
        }
    }

    pub(in crate::desktop_shell) fn group_menu<'a>(
        &'a self,
        topic: GroupMenuTopic,
        groups: impl IntoIterator<Item = WidgetGroupContent<'a, Message>>,
    ) -> GroupMenuView<'a, Message> {
        GroupMenuView::new(
            topic,
            &self.widget_groups.group_menus,
            Message::GroupMenuState,
            groups,
        )
    }

    fn network_group_menu(&self) -> GroupMenuView<'_, Message> {
        let groups = self
            .widget_groups
            .wifi_adapters_group
            .group_contents()
            .into_iter()
            .map(|group| group.map(Message::WifiAdapters))
            .chain(
                self.widget_groups
                    .mobile_modems_group
                    .group_contents()
                    .into_iter()
                    .map(|group| group.map(Message::MobileModems)),
            )
            .chain(
                self.widget_groups
                    .ethernet_ports_group
                    .group_contents()
                    .into_iter()
                    .map(|group| group.map(Message::EthernetPorts)),
            )
            .chain(
                self.widget_groups
                    .bluetooth_adapters_group
                    .group_contents(
                        &|hardware_address| {
                            self.widget_groups
                                .gamepad_group
                                .report_timing_for_address(hardware_address)
                        },
                        self.clock,
                    )
                    .into_iter()
                    .map(|group| group.map(Message::BluetoothAdapters)),
            )
            .chain([
                self.widget_groups
                    .reachability_group
                    .group_content()
                    .map(Message::Reachability),
                self.widget_groups
                    .tailnet_group
                    .group_content()
                    .map(Message::Tailnet),
                self.widget_groups
                    .wireguard_tunnels_group
                    .group_content()
                    .map(Message::WireguardTunnels),
                self.widget_groups
                    .network_manager_vpn_group
                    .group_content()
                    .map(Message::NetworkManagerVpns),
            ]);
        self.group_menu(GroupMenuTopic::Network, groups)
    }

    fn hardware_group_menu(&self) -> GroupMenuView<'_, Message> {
        let config = &self.config;
        let groups = [
            self.widget_groups
                .power_group
                .group_content(config)
                .map(Message::Power),
            self.widget_groups
                .compute_group
                .group_content()
                .map(Message::Compute),
            self.widget_groups
                .memory_board_group
                .group_content()
                .map(Message::MemoryBoard),
        ]
        .into_iter()
        .chain(
            self.widget_groups
                .gpu_group
                .group_contents()
                .map(|group| group.map(Message::Gpu)),
        )
        .chain(
            self.widget_groups
                .storage_space_group
                .group_contents()
                .into_iter()
                .map(|group| group.map(Message::Storage)),
        )
        .chain(
            self.widget_groups
                .physical_drive_group
                .group_contents()
                .into_iter()
                .map(|group| group.map(Message::Drive)),
        );
        self.group_menu(GroupMenuTopic::Hardware, groups)
    }

    fn agenda_group_menu(&self) -> GroupMenuView<'_, Message> {
        let config = &self.config;
        self.group_menu(
            GroupMenuTopic::Agenda,
            [
                self.widget_groups
                    .caldav_group
                    .group_content(&config.widget_groups.caldav)
                    .map(Message::Caldav),
                self.widget_groups
                    .weather_group
                    .group_content(config)
                    .map(Message::Weather),
                self.widget_groups
                    .quiet_schedule_group
                    .group_content(config)
                    .map(Message::QuietSchedule),
                self.widget_groups
                    .session_focus_group
                    .group_content(config)
                    .map(Message::SessionFocus),
            ],
        )
    }

    fn media_group_menu(&self) -> GroupMenuView<'_, Message> {
        let config = &self.config;
        let media_groups = self
            .widget_groups
            .media_group
            .group_contents(config, self.clock)
            .map(|group| group.map(Message::Media));
        let gamepad_groups = self
            .widget_groups
            .gamepad_group
            .group_contents(&config.widget_groups.gamepad)
            .into_iter()
            .map(|group| group.map(Message::Gamepad));
        let groups = media_groups
            .chain([
                self.widget_groups
                    .audio_group
                    .group_content(config)
                    .map(Message::Audio),
                self.widget_groups
                    .voice_dictation_group
                    .group_content()
                    .map(Message::VoiceDictation),
            ])
            .chain(gamepad_groups);
        self.group_menu(GroupMenuTopic::Media, groups)
    }

    fn settings_groups(&self) -> [WidgetGroupContent<'_, Message>; 6] {
        let config = &self.config;
        [
            self.widget_groups
                .panel_settings_group
                .group_content(config)
                .map(Message::PanelSettings),
            self.widget_groups
                .sidebar_settings_group
                .group_content(config)
                .map(Message::SidebarSettings),
            self.widget_groups
                .overlay_settings_group
                .group_content(config)
                .map(Message::OverlaySettings),
            self.widget_groups
                .compositor_styling_group
                .group_content(config)
                .map(Message::CompositorStyling),
            self.widget_groups
                .group_menus
                .settings_content(config)
                .map(Message::GroupMenuState),
            self.widget_groups
                .global_menu_group
                .switch_content()
                .map(Message::GlobalMenu),
        ]
    }

    fn display_group_menu(&self) -> GroupMenuView<'_, Message> {
        let config = &self.config;
        self.group_menu(
            GroupMenuTopic::Display,
            [
                self.widget_groups
                    .monitor_group
                    .group_content(config)
                    .map(Message::Monitor),
                self.widget_groups
                    .dark_mode_group
                    .group_content(config)
                    .map(Message::DarkMode),
                self.widget_groups
                    .color_temperature_group
                    .group_content(config)
                    .map(Message::ColorTemperature),
                self.widget_groups
                    .wallpaper_group
                    .group_content()
                    .map(Message::Wallpaper),
            ],
        )
    }

    fn session_group_menu(&self) -> GroupMenuView<'_, Message> {
        let pause_groups = self
            .widget_groups
            .idle_pause_groups
            .pause_groups(&self.config)
            .into_iter()
            .map(|group| group.map(Message::IdleTimeouts));
        let login_session = self
            .widget_groups
            .login_session_group
            .group_content()
            .map(Message::LoginSession);
        self.group_menu(
            GroupMenuTopic::Session,
            pause_groups.chain(once(login_session)),
        )
    }

    fn virtualization_group_menu(&self) -> GroupMenuView<'_, Message> {
        self.group_menu(
            GroupMenuTopic::Virtualization,
            [
                self.widget_groups
                    .virtual_machines_group
                    .group_content()
                    .map(Message::VirtualMachines),
                self.widget_groups
                    .virtualbox_machines_group
                    .group_content()
                    .map(Message::VirtualBoxMachines),
                self.widget_groups
                    .vmware_machines_group
                    .group_content()
                    .map(Message::VmwareMachines),
                self.widget_groups
                    .microvm_guests_group
                    .group_content()
                    .map(Message::MicrovmGuests),
                self.widget_groups
                    .incus_instances_group
                    .group_content()
                    .map(Message::IncusInstances),
                self.widget_groups
                    .nspawn_containers_group
                    .group_content()
                    .map(Message::NspawnContainers),
                self.widget_groups
                    .lxc_containers_group
                    .group_content()
                    .map(Message::LxcContainers),
                self.widget_groups
                    .podman_engines_group
                    .group_content()
                    .map(Message::PodmanEngines),
                self.widget_groups
                    .docker_engines_group
                    .group_content()
                    .map(Message::DockerEngines),
                self.widget_groups
                    .apptainer_containers_group
                    .group_content()
                    .map(Message::ApptainerContainers),
                self.widget_groups
                    .waydroid_container_group
                    .group_content()
                    .map(Message::WaydroidContainer),
                self.widget_groups
                    .flatpak_sandboxes_group
                    .group_content()
                    .map(Message::FlatpakSandboxes),
                self.widget_groups
                    .cgroup_memory_limits_group
                    .group_content(&self.config)
                    .map(Message::CgroupMemoryLimits),
            ],
        )
    }

    fn services_group_menu(&self) -> GroupMenuView<'_, Message> {
        let config = &self.config;
        self.group_menu(
            GroupMenuTopic::Services,
            [
                self.widget_groups
                    .llama_server_group
                    .group_content()
                    .map(Message::LlamaServer),
                self.widget_groups
                    .ci_runs_group
                    .group_content()
                    .map(Message::CiRuns),
                self.widget_groups
                    .claude_usage_group
                    .group_content()
                    .map(Message::ClaudeUsage),
                self.widget_groups
                    .copilot_usage_group
                    .group_content()
                    .map(Message::CopilotUsage),
                self.widget_groups
                    .openrouter_credit_group
                    .group_content()
                    .map(Message::OpenRouterCredit),
                self.widget_groups
                    .systemd_manager_group
                    .group_content(config, self.clock)
                    .map(Message::SystemdManager),
                self.widget_groups
                    .journal_events_group
                    .group_content(config)
                    .map(Message::JournalEvents),
                self.widget_groups
                    .pending_upgrades_group
                    .group_content(config)
                    .map(Message::PendingUpgrades),
            ],
        )
    }

    pub(in crate::desktop_shell) fn notification_status(&self) -> NotificationStatus {
        NotificationStatus {
            quiet_phase: self.widget_groups.quiet_schedule_group.quiet_phase(),
            live: self.notifications.live.len(),
            held: self.notifications.held.len(),
        }
    }
}
