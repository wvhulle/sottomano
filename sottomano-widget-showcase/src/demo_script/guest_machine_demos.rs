use sottomano_widgets::{
    group_indicators::indicator_group_identity::IndicatorGroupFamily,
    group_menus::group_menu_state::GroupMenuTopic, hover_cards::HoverTarget,
};

use crate::{
    demo_script::{Demo, DemoBackdrop, GroupShown, Step, UiPart, indicator_row},
    fixtures::{
        apptainer_containers, cgroup_memory_limits, ci_runner_tasks, flatpak_sandboxes,
        lxc_containers, microvm_guests, unchanging, virtualbox_machines, vmware_machines,
        waydroid_container,
    },
};

pub(super) const MICROVM_GUESTS_CLOUD_HYPERVISOR: Demo = Demo {
    name: "microvm_guests_cloud_hypervisor",
    parts: &[UiPart::GroupMenus, UiPart::HoverCards],
    group: microvm_guests::group,
    live: |_| unchanging(),
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
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
    parts: &[UiPart::GroupMenus, UiPart::HoverCards],
    group: lxc_containers::group,
    live: |_| unchanging(),
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
        Step::PressText("LXC containers"),
        Step::Hover(indicator_row(IndicatorGroupFamily::LxcContainers, "media")),
    ],
};

pub(super) const FLATPAK_SANDBOXES_APP: Demo = Demo {
    name: "flatpak_sandboxes_app",
    parts: &[UiPart::GroupMenus, UiPart::HoverCards],
    group: flatpak_sandboxes::group,
    live: |_| unchanging(),
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
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
    parts: &[UiPart::GroupMenus, UiPart::HoverCards],
    group: waydroid_container::group,
    live: |_| unchanging(),
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
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
    parts: &[UiPart::GroupMenus, UiPart::HoverCards],
    group: virtualbox_machines::group,
    live: |_| unchanging(),
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
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
    parts: &[UiPart::GroupMenus, UiPart::HoverCards],
    group: vmware_machines::group,
    live: |_| unchanging(),
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
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
    parts: &[UiPart::GroupMenus, UiPart::HoverCards],
    group: apptainer_containers::group,
    live: |_| unchanging(),
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
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
    parts: &[UiPart::GroupMenus, UiPart::HoverCards],
    group: cgroup_memory_limits::group,
    live: |_| unchanging(),
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
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
    parts: &[UiPart::GroupMenus, UiPart::HoverCards],
    group: ci_runner_tasks::group,
    live: |_| unchanging(),
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Services),
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Services)),
        Step::PressText("3 CI runs"),
        Step::Hover(indicator_row(IndicatorGroupFamily::CiRuns, "website #185")),
    ],
};
