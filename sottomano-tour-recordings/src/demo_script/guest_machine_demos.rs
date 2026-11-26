use sottomano_widget_scenes::{
    scenes::guest_machine_scenes,
    script_steps::{Step, indicator_row},
};
use sottomano_widgets::{
    group_indicators::indicator_group_identity::IndicatorGroupFamily,
    group_menus::group_menu_state::GroupMenuTopic, hover_cards::HoverTarget,
};

use crate::demo_script::Demo;

pub(super) const MICROVM_GUESTS_CLOUD_HYPERVISOR: Demo = Demo {
    name: "microvm_guests_cloud_hypervisor",
    scene: &guest_machine_scenes::MICROVM_GUESTS_CLOUD_HYPERVISOR,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
        Step::PressText("microvm.nix guests"),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::MicrovmGuests,
            "ci-builder",
        )),
    ],
};

pub(super) const LXC_CONTAINERS_RUNNING: Demo = Demo {
    name: "lxc_containers_running",
    scene: &guest_machine_scenes::LXC_CONTAINERS_RUNNING,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
        Step::PressText("LXC containers"),
        Step::Hover(indicator_row(IndicatorGroupFamily::LxcContainers, "media")),
    ],
};

pub(super) const FLATPAK_SANDBOXES_APP: Demo = Demo {
    name: "flatpak_sandboxes_app",
    scene: &guest_machine_scenes::FLATPAK_SANDBOXES_APP,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
        Step::PressText("Flatpak apps"),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::FlatpakSandboxes,
            "com.spotify.Client 1849302754",
        )),
    ],
};

pub(super) const WAYDROID_CONTAINER_SESSION: Demo = Demo {
    name: "waydroid_container_session",
    scene: &guest_machine_scenes::WAYDROID_CONTAINER_SESSION,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
        Step::PressText("Waydroid"),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::WaydroidContainer,
            "Session",
        )),
    ],
};

pub(super) const VIRTUALBOX_MACHINES_HEADLESS: Demo = Demo {
    name: "virtualbox_machines_headless",
    scene: &guest_machine_scenes::VIRTUALBOX_MACHINES_HEADLESS,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
        Step::PressText("VirtualBox machines"),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::VirtualBoxMachines,
            "windows11 pid 61402",
        )),
    ],
};

pub(super) const VMWARE_MACHINES_WORKSTATION: Demo = Demo {
    name: "vmware_machines_workstation",
    scene: &guest_machine_scenes::VMWARE_MACHINES_WORKSTATION,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
        Step::PressText("VMware machines"),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::VmwareMachines,
            "Ubuntu 26.04",
        )),
    ],
};

pub(super) const APPTAINER_CONTAINERS_IMAGE: Demo = Demo {
    name: "apptainer_containers_image",
    scene: &guest_machine_scenes::APPTAINER_CONTAINERS_IMAGE,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
        Step::PressText("Apptainer containers"),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::ApptainerContainers,
            "pytorch_24.08 pid 70215",
        )),
    ],
};

pub(super) const CGROUP_MEMORY_LIMITS_CI_RUNNERS: Demo = Demo {
    name: "cgroup_memory_limits_ci_runners",
    scene: &guest_machine_scenes::CGROUP_MEMORY_LIMITS_CI_RUNNERS,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
        Step::PressText("Memory limits"),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::CgroupMemoryLimits,
            "ci-runners.slice",
        )),
    ],
};

pub(super) const CI_RUNNER_TASKS: Demo = Demo {
    name: "ci_runner_tasks",
    scene: &guest_machine_scenes::CI_RUNNER_TASKS,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Services)),
        Step::PressText("3 CI runs"),
        Step::Hover(indicator_row(IndicatorGroupFamily::CiRuns, "website #185")),
    ],
};
