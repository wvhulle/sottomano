use super::IndicatorGroupInteraction;
use crate::panel_menus::virtualization_menu::{
    apptainer_containers_group::ApptainerContainersInteraction,
    cgroup_memory_limits_group::CgroupMemoryLimitsInteraction,
    flatpak_sandboxes_group::FlatpakSandboxesInteraction,
    incus_instances_group::IncusInstancesInteraction,
    lxc_containers_group::LxcContainersInteraction, microvm_guests_group::MicrovmGuestsInteraction,
    nspawn_containers_group::NspawnContainersInteraction,
    virtual_machines_group::VirtualMachinesInteraction,
    virtualbox_machines_group::VirtualBoxMachinesInteraction,
    vmware_machines_group::VmwareMachinesInteraction,
    waydroid_container_group::WaydroidContainerInteraction,
};

impl From<VirtualMachinesInteraction> for IndicatorGroupInteraction {
    fn from(interaction: VirtualMachinesInteraction) -> Self {
        IndicatorGroupInteraction::VirtualMachines(interaction)
    }
}

impl From<NspawnContainersInteraction> for IndicatorGroupInteraction {
    fn from(interaction: NspawnContainersInteraction) -> Self {
        IndicatorGroupInteraction::NspawnContainers(interaction)
    }
}

impl From<CgroupMemoryLimitsInteraction> for IndicatorGroupInteraction {
    fn from(interaction: CgroupMemoryLimitsInteraction) -> Self {
        IndicatorGroupInteraction::CgroupMemoryLimits(interaction)
    }
}

impl From<ApptainerContainersInteraction> for IndicatorGroupInteraction {
    fn from(interaction: ApptainerContainersInteraction) -> Self {
        IndicatorGroupInteraction::ApptainerContainers(interaction)
    }
}

impl From<VmwareMachinesInteraction> for IndicatorGroupInteraction {
    fn from(interaction: VmwareMachinesInteraction) -> Self {
        IndicatorGroupInteraction::VmwareMachines(interaction)
    }
}

impl From<VirtualBoxMachinesInteraction> for IndicatorGroupInteraction {
    fn from(interaction: VirtualBoxMachinesInteraction) -> Self {
        IndicatorGroupInteraction::VirtualBoxMachines(interaction)
    }
}

impl From<WaydroidContainerInteraction> for IndicatorGroupInteraction {
    fn from(interaction: WaydroidContainerInteraction) -> Self {
        IndicatorGroupInteraction::WaydroidContainer(interaction)
    }
}

impl From<FlatpakSandboxesInteraction> for IndicatorGroupInteraction {
    fn from(interaction: FlatpakSandboxesInteraction) -> Self {
        IndicatorGroupInteraction::FlatpakSandboxes(interaction)
    }
}

impl From<LxcContainersInteraction> for IndicatorGroupInteraction {
    fn from(interaction: LxcContainersInteraction) -> Self {
        IndicatorGroupInteraction::LxcContainers(interaction)
    }
}

impl From<IncusInstancesInteraction> for IndicatorGroupInteraction {
    fn from(interaction: IncusInstancesInteraction) -> Self {
        IndicatorGroupInteraction::IncusInstances(interaction)
    }
}

impl From<MicrovmGuestsInteraction> for IndicatorGroupInteraction {
    fn from(interaction: MicrovmGuestsInteraction) -> Self {
        IndicatorGroupInteraction::MicrovmGuests(interaction)
    }
}
