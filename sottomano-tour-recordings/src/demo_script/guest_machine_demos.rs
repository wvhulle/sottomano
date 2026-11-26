use sottomano_widget_scenes::{
    scenes::guest_machine_scenes,
    script_steps::{Step, indicator_row},
};
use sottomano_widgets::{
    group_indicators::indicator_group_identity::IndicatorGroupFamily,
    group_menus::group_menu_state::GroupMenuTopic, hover_cards::HoverTarget,
};

use crate::demo_script::Demo;

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

pub(super) const MEMORY_LIMITS_SLICE: Demo = Demo {
    name: "memory_limits_slice",
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

pub(super) const CI_RUNS_TASK: Demo = Demo {
    name: "ci_runs_task",
    scene: &guest_machine_scenes::CI_RUNNER_TASKS,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Services)),
        Step::PressText("3 CI runs"),
        Step::Hover(indicator_row(IndicatorGroupFamily::CiRuns, "website #185")),
    ],
};
