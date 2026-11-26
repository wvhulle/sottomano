use sottomano_widgets::group_menus::group_menu_state::GroupMenuTopic;

use crate::{
    fixtures::{
        apptainer_containers, cgroup_memory_limits, ci_runner_tasks, flatpak_sandboxes,
        lxc_containers, microvm_guests, unchanging, virtualbox_machines, vmware_machines,
        waydroid_container,
    },
    scenes::{GroupShown, Scene, SceneBackdrop},
};

pub static MICROVM_GUESTS_CLOUD_HYPERVISOR: Scene = Scene {
    name: "microvm_guests_cloud_hypervisor",
    group: microvm_guests::group,
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
};

pub static LXC_CONTAINERS_RUNNING: Scene = Scene {
    name: "lxc_containers_running",
    group: lxc_containers::group,
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
};

pub static FLATPAK_SANDBOXES_APP: Scene = Scene {
    name: "flatpak_sandboxes_app",
    group: flatpak_sandboxes::group,
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
};

pub static WAYDROID_CONTAINER_SESSION: Scene = Scene {
    name: "waydroid_container_session",
    group: waydroid_container::group,
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
};

pub static VIRTUALBOX_MACHINES_HEADLESS: Scene = Scene {
    name: "virtualbox_machines_headless",
    group: virtualbox_machines::group,
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
};

pub static VMWARE_MACHINES_WORKSTATION: Scene = Scene {
    name: "vmware_machines_workstation",
    group: vmware_machines::group,
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
};

pub static APPTAINER_CONTAINERS_IMAGE: Scene = Scene {
    name: "apptainer_containers_image",
    group: apptainer_containers::group,
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
};

pub static CGROUP_MEMORY_LIMITS_CI_RUNNERS: Scene = Scene {
    name: "cgroup_memory_limits_ci_runners",
    group: cgroup_memory_limits::group,
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
};

pub static CI_RUNNER_TASKS: Scene = Scene {
    name: "ci_runner_tasks",
    group: ci_runner_tasks::group,
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Services),
};
