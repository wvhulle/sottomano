pub mod apptainer_containers_group;
pub mod cgroup_memory_limits_group;
pub mod container_family_group;
pub mod docker_engines_group;
pub mod engine_container_indicators;
pub mod flatpak_sandboxes_group;
pub mod incus_instances_group;
pub mod lxc_containers_group;
pub mod microvm_guests_group;
pub mod nspawn_containers_group;
pub mod podman_engines_group;
pub mod virtual_machines_group;
pub mod virtualbox_machines_group;
pub mod vmware_machines_group;
pub mod waydroid_container_group;

use sottomano_toml_config::Config;

use self::{
    apptainer_containers_group::ApptainerContainersGroup,
    cgroup_memory_limits_group::CgroupMemoryLimitsGroup, docker_engines_group::DockerEnginesGroup,
    flatpak_sandboxes_group::FlatpakSandboxesGroup, incus_instances_group::IncusInstancesGroup,
    lxc_containers_group::LxcContainersGroup, microvm_guests_group::MicrovmGuestsGroup,
    nspawn_containers_group::NspawnContainersGroup, podman_engines_group::PodmanEnginesGroup,
    virtual_machines_group::VirtualMachinesGroup,
    virtualbox_machines_group::VirtualBoxMachinesGroup, vmware_machines_group::VmwareMachinesGroup,
    waydroid_container_group::WaydroidContainerGroup,
};
use crate::group_indicators::indicator_group_content::IndicatorGroupContent;

#[derive(Default)]
pub struct VirtualizationMenu {
    pub virtual_machines_group: VirtualMachinesGroup,
    pub virtualbox_machines_group: VirtualBoxMachinesGroup,
    pub vmware_machines_group: VmwareMachinesGroup,
    pub microvm_guests_group: MicrovmGuestsGroup,
    pub incus_instances_group: IncusInstancesGroup,
    pub nspawn_containers_group: NspawnContainersGroup,
    pub lxc_containers_group: LxcContainersGroup,
    pub podman_engines_group: PodmanEnginesGroup,
    pub docker_engines_group: DockerEnginesGroup,
    pub apptainer_containers_group: ApptainerContainersGroup,
    pub waydroid_container_group: WaydroidContainerGroup,
    pub flatpak_sandboxes_group: FlatpakSandboxesGroup,
    pub cgroup_memory_limits_group: CgroupMemoryLimitsGroup,
}

impl VirtualizationMenu {
    pub fn collapse(&mut self) {
        self.virtual_machines_group.collapse();
        self.virtualbox_machines_group.collapse();
        self.vmware_machines_group.collapse();
        self.microvm_guests_group.collapse();
        self.incus_instances_group.collapse();
        self.nspawn_containers_group.collapse();
        self.lxc_containers_group.collapse();
        self.podman_engines_group.collapse();
        self.docker_engines_group.collapse();
        self.apptainer_containers_group.collapse();
        self.waydroid_container_group.collapse();
        self.flatpak_sandboxes_group.collapse();
        self.cgroup_memory_limits_group.collapse();
    }

    #[must_use]
    pub fn group_contents<'view>(
        &'view self,
        config: &'view Config,
    ) -> Vec<IndicatorGroupContent<'view>> {
        vec![
            self.virtual_machines_group.group_content(),
            self.virtualbox_machines_group.group_content(),
            self.vmware_machines_group.group_content(),
            self.microvm_guests_group.group_content(),
            self.incus_instances_group.group_content(),
            self.nspawn_containers_group.group_content(),
            self.lxc_containers_group.group_content(),
            self.podman_engines_group.group_content(),
            self.docker_engines_group.group_content(),
            self.apptainer_containers_group.group_content(),
            self.waydroid_container_group.group_content(),
            self.flatpak_sandboxes_group.group_content(),
            self.cgroup_memory_limits_group.group_content(config),
        ]
    }
}
