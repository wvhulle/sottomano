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
    shell_message::ShellInteraction,
};

impl DesktopShell {
    pub(in crate::desktop_shell) fn group_menu_of(
        &self,
        topic: GroupMenuTopic,
        output: &str,
    ) -> GroupMenuView<'_, ShellInteraction> {
        match topic {
            GroupMenuTopic::Applications => self.group_menu(
                GroupMenuTopic::Applications,
                self.widget_groups
                    .app_launcher_group
                    .group_contents(&self.desktop_entries, &self.config, output)
                    .into_iter()
                    .map(|group| group.map(ShellInteraction::AppLauncher)),
            ),
            GroupMenuTopic::ApplicationMenu => self.group_menu(
                GroupMenuTopic::ApplicationMenu,
                self.widget_groups
                    .global_menu_group
                    .group_contents()
                    .into_iter()
                    .map(|group| group.map(ShellInteraction::GlobalMenu)),
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
                            .map(ShellInteraction::AppearancePresets),
                    )),
            ),
        }
    }

    pub(in crate::desktop_shell) fn fixed_panel_group(
        &self,
        entry: FixedPanelEntry,
    ) -> FixedPanelGroup<'_, ShellInteraction> {
        match entry {
            FixedPanelEntry::SessionPower => self
                .widget_groups
                .session_power_group
                .group_content()
                .map(ShellInteraction::Session),
        }
    }

    pub(in crate::desktop_shell) fn standalone_group(
        &self,
        group: StandaloneWidgetGroup,
    ) -> WidgetGroupContent<'_, ShellInteraction> {
        let config = &self.config;
        let window_placement = Placement::derive(&self.compositor);
        match group {
            StandaloneWidgetGroup::Clock => self
                .widget_groups
                .clock_group
                .group_content()
                .map(ShellInteraction::Clock),
            StandaloneWidgetGroup::Notifications => self
                .widget_groups
                .notification_group
                .group_content(self.notification_status())
                .map(ShellInteraction::NotificationGroup),
            StandaloneWidgetGroup::Tray => self
                .widget_groups
                .tray_group
                .group_content(config, &|owner| {
                    window_placement.window_owned_by(owner).is_some()
                })
                .map(ShellInteraction::Tray),
        }
    }

    pub(in crate::desktop_shell) fn group_menu<'a>(
        &'a self,
        topic: GroupMenuTopic,
        groups: impl IntoIterator<Item = WidgetGroupContent<'a, ShellInteraction>>,
    ) -> GroupMenuView<'a, ShellInteraction> {
        GroupMenuView::new(
            topic,
            &self.widget_groups.group_menus,
            ShellInteraction::GroupMenuState,
            groups,
        )
    }

    fn network_group_menu(&self) -> GroupMenuView<'_, ShellInteraction> {
        let groups = self
            .widget_groups
            .wifi_adapters_group
            .group_contents()
            .into_iter()
            .map(|group| group.map(ShellInteraction::WifiAdapters))
            .chain(
                self.widget_groups
                    .mobile_modems_group
                    .group_contents()
                    .into_iter()
                    .map(|group| group.map(ShellInteraction::MobileModems)),
            )
            .chain(
                self.widget_groups
                    .ethernet_ports_group
                    .group_contents()
                    .into_iter()
                    .map(|group| group.map(ShellInteraction::EthernetPorts)),
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
                    .map(|group| group.map(ShellInteraction::BluetoothAdapters)),
            )
            .chain([
                self.widget_groups
                    .reachability_group
                    .group_content()
                    .map(ShellInteraction::Reachability),
                self.widget_groups
                    .tailnet_group
                    .group_content()
                    .map(ShellInteraction::Tailnet),
                self.widget_groups
                    .wireguard_tunnels_group
                    .group_content()
                    .map(ShellInteraction::WireguardTunnels),
                self.widget_groups
                    .network_manager_vpn_group
                    .group_content()
                    .map(ShellInteraction::NetworkManagerVpns),
            ]);
        self.group_menu(GroupMenuTopic::Network, groups)
    }

    fn hardware_group_menu(&self) -> GroupMenuView<'_, ShellInteraction> {
        let config = &self.config;
        let groups = [
            self.widget_groups
                .power_group
                .group_content(config)
                .map(ShellInteraction::Power),
            self.widget_groups
                .compute_group
                .group_content()
                .map(ShellInteraction::Compute),
            self.widget_groups
                .memory_board_group
                .group_content()
                .map(ShellInteraction::MemoryBoard),
        ]
        .into_iter()
        .chain(
            self.widget_groups
                .gpu_group
                .group_contents()
                .map(|group| group.map(ShellInteraction::Gpu)),
        )
        .chain(
            self.widget_groups
                .storage_space_group
                .group_contents()
                .into_iter()
                .map(|group| group.map(ShellInteraction::Storage)),
        )
        .chain(
            self.widget_groups
                .physical_drive_group
                .group_contents()
                .into_iter()
                .map(|group| group.map(ShellInteraction::Drive)),
        );
        self.group_menu(GroupMenuTopic::Hardware, groups)
    }

    fn agenda_group_menu(&self) -> GroupMenuView<'_, ShellInteraction> {
        let config = &self.config;
        self.group_menu(
            GroupMenuTopic::Agenda,
            [
                self.widget_groups
                    .caldav_group
                    .group_content(&config.widget_groups.caldav)
                    .map(ShellInteraction::Caldav),
                self.widget_groups
                    .weather_group
                    .group_content(config)
                    .map(ShellInteraction::Weather),
                self.widget_groups
                    .quiet_schedule_group
                    .group_content(config)
                    .map(ShellInteraction::QuietSchedule),
                self.widget_groups
                    .session_focus_group
                    .group_content(config)
                    .map(ShellInteraction::SessionFocus),
            ],
        )
    }

    fn media_group_menu(&self) -> GroupMenuView<'_, ShellInteraction> {
        let config = &self.config;
        let media_groups = self
            .widget_groups
            .media_group
            .group_contents(config, self.clock)
            .map(|group| group.map(ShellInteraction::Media));
        let gamepad_groups = self
            .widget_groups
            .gamepad_group
            .group_contents(&config.widget_groups.gamepad)
            .into_iter()
            .map(|group| group.map(ShellInteraction::Gamepad));
        let groups = media_groups
            .chain([
                self.widget_groups
                    .audio_group
                    .group_content(config)
                    .map(ShellInteraction::Audio),
                self.widget_groups
                    .voice_dictation_group
                    .group_content()
                    .map(ShellInteraction::VoiceDictation),
            ])
            .chain(gamepad_groups);
        self.group_menu(GroupMenuTopic::Media, groups)
    }

    fn settings_groups(&self) -> [WidgetGroupContent<'_, ShellInteraction>; 6] {
        let config = &self.config;
        [
            self.widget_groups
                .panel_settings_group
                .group_content(config)
                .map(ShellInteraction::PanelSettings),
            self.widget_groups
                .sidebar_settings_group
                .group_content(config)
                .map(ShellInteraction::SidebarSettings),
            self.widget_groups
                .overlay_settings_group
                .group_content(config)
                .map(ShellInteraction::OverlaySettings),
            self.widget_groups
                .compositor_styling_group
                .group_content(config)
                .map(ShellInteraction::CompositorStyling),
            self.widget_groups
                .group_menus
                .settings_content(config)
                .map(ShellInteraction::GroupMenuState),
            self.widget_groups
                .global_menu_group
                .switch_content()
                .map(ShellInteraction::GlobalMenu),
        ]
    }

    fn display_group_menu(&self) -> GroupMenuView<'_, ShellInteraction> {
        let config = &self.config;
        self.group_menu(
            GroupMenuTopic::Display,
            [
                self.widget_groups
                    .monitor_group
                    .group_content(config)
                    .map(ShellInteraction::Monitor),
                self.widget_groups
                    .dark_mode_group
                    .group_content(config)
                    .map(ShellInteraction::DarkMode),
                self.widget_groups
                    .color_temperature_group
                    .group_content(config)
                    .map(ShellInteraction::ColorTemperature),
                self.widget_groups
                    .wallpaper_group
                    .group_content()
                    .map(ShellInteraction::Wallpaper),
            ],
        )
    }

    fn session_group_menu(&self) -> GroupMenuView<'_, ShellInteraction> {
        let pause_groups = self
            .widget_groups
            .idle_pause_groups
            .pause_groups(&self.config)
            .into_iter()
            .map(|group| group.map(ShellInteraction::IdleTimeouts));
        let login_session = self
            .widget_groups
            .login_session_group
            .group_content()
            .map(ShellInteraction::LoginSession);
        self.group_menu(
            GroupMenuTopic::Session,
            pause_groups.chain(once(login_session)),
        )
    }

    fn virtualization_group_menu(&self) -> GroupMenuView<'_, ShellInteraction> {
        self.group_menu(
            GroupMenuTopic::Virtualization,
            [
                self.widget_groups
                    .virtual_machines_group
                    .group_content()
                    .map(ShellInteraction::VirtualMachines),
                self.widget_groups
                    .virtualbox_machines_group
                    .group_content()
                    .map(ShellInteraction::VirtualBoxMachines),
                self.widget_groups
                    .vmware_machines_group
                    .group_content()
                    .map(ShellInteraction::VmwareMachines),
                self.widget_groups
                    .microvm_guests_group
                    .group_content()
                    .map(ShellInteraction::MicrovmGuests),
                self.widget_groups
                    .incus_instances_group
                    .group_content()
                    .map(ShellInteraction::IncusInstances),
                self.widget_groups
                    .nspawn_containers_group
                    .group_content()
                    .map(ShellInteraction::NspawnContainers),
                self.widget_groups
                    .lxc_containers_group
                    .group_content()
                    .map(ShellInteraction::LxcContainers),
                self.widget_groups
                    .podman_engines_group
                    .group_content()
                    .map(ShellInteraction::PodmanEngines),
                self.widget_groups
                    .docker_engines_group
                    .group_content()
                    .map(ShellInteraction::DockerEngines),
                self.widget_groups
                    .apptainer_containers_group
                    .group_content()
                    .map(ShellInteraction::ApptainerContainers),
                self.widget_groups
                    .waydroid_container_group
                    .group_content()
                    .map(ShellInteraction::WaydroidContainer),
                self.widget_groups
                    .flatpak_sandboxes_group
                    .group_content()
                    .map(ShellInteraction::FlatpakSandboxes),
                self.widget_groups
                    .cgroup_memory_limits_group
                    .group_content(&self.config)
                    .map(ShellInteraction::CgroupMemoryLimits),
            ],
        )
    }

    fn services_group_menu(&self) -> GroupMenuView<'_, ShellInteraction> {
        let config = &self.config;
        self.group_menu(
            GroupMenuTopic::Services,
            [
                self.widget_groups
                    .llama_server_group
                    .group_content()
                    .map(ShellInteraction::LlamaServer),
                self.widget_groups
                    .ci_runs_group
                    .group_content()
                    .map(ShellInteraction::CiRuns),
                self.widget_groups
                    .claude_usage_group
                    .group_content()
                    .map(ShellInteraction::ClaudeUsage),
                self.widget_groups
                    .copilot_usage_group
                    .group_content()
                    .map(ShellInteraction::CopilotUsage),
                self.widget_groups
                    .openrouter_credit_group
                    .group_content()
                    .map(ShellInteraction::OpenRouterCredit),
                self.widget_groups
                    .systemd_manager_group
                    .group_content(config, self.clock)
                    .map(ShellInteraction::SystemdManager),
                self.widget_groups
                    .journal_events_group
                    .system_journal_content(config)
                    .map(ShellInteraction::JournalEvents),
                self.widget_groups
                    .journal_events_group
                    .user_journal_content(config)
                    .map(ShellInteraction::JournalEvents),
                self.widget_groups
                    .pending_upgrades_group
                    .group_content(config)
                    .map(ShellInteraction::PendingUpgrades),
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
