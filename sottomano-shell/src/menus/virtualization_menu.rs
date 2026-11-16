use sottomano_widgets::group_menus::{
    group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView,
};

use crate::desktop_shell::DesktopShell;

impl DesktopShell {
    pub(crate) fn virtualization_group_menu(&self) -> GroupMenuView<'_> {
        self.group_menu(
            GroupMenuTopic::Virtualization,
            [
                self.indicator_groups.virtual_machines_group.group_content(),
                self.indicator_groups
                    .virtualbox_machines_group
                    .group_content(),
                self.indicator_groups.vmware_machines_group.group_content(),
                self.indicator_groups.microvm_guests_group.group_content(),
                self.indicator_groups.incus_instances_group.group_content(),
                self.indicator_groups
                    .nspawn_containers_group
                    .group_content(),
                self.indicator_groups.lxc_containers_group.group_content(),
                self.indicator_groups.podman_engines_group.group_content(),
                self.indicator_groups.docker_engines_group.group_content(),
                self.indicator_groups
                    .apptainer_containers_group
                    .group_content(),
                self.indicator_groups
                    .waydroid_container_group
                    .group_content(),
                self.indicator_groups
                    .flatpak_sandboxes_group
                    .group_content(),
                self.indicator_groups
                    .cgroup_memory_limits_group
                    .group_content(&self.config),
            ],
        )
    }
}
