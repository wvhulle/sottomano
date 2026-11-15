use sottomano_widgets::group_menus::{
    group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView,
};

use crate::{desktop_shell::DesktopShell, shell_message::ShellInteraction};

impl DesktopShell {
    pub(crate) fn virtualization_group_menu(&self) -> GroupMenuView<'_, ShellInteraction> {
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
}
