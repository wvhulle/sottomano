use std::time::Duration;

use sottomano_widget_scenes::{scenes::desktop_session_scenes, script_steps::Step};
use sottomano_widgets::{hover_cards::HoverTarget, output_windows::WindowTarget};

use crate::demo_script::Demo;

pub(super) const BOTTOM_WORKSPACES: Demo = Demo {
    name: "bottom_workspaces",
    scene: &desktop_session_scenes::BOTTOM_BAR,
    script: &[Step::Hover(HoverTarget::Workspace(2))],
};

pub(super) const BOTTOM_WINDOW_DIRECTORY_GROUPS: Demo = Demo {
    name: "bottom_window_directory_groups",
    scene: &desktop_session_scenes::BOTTOM_WINDOW_DIRECTORY_GROUPS,
    script: &[Step::Hover(HoverTarget::Window(WindowTarget::Usage(2)))],
};

pub(super) const BOTTOM_PROCESS_INSPECTOR: Demo = Demo {
    name: "bottom_process_inspector",
    scene: &desktop_session_scenes::BOTTOM_BAR,
    script: &[Step::Hover(HoverTarget::Window(WindowTarget::Inspect(2)))],
};

pub(super) const TOAST_ARRIVAL: Demo = Demo {
    name: "toast_arrival",
    scene: &desktop_session_scenes::TOAST_ARRIVAL,
    script: &[
        Step::HoverText(desktop_session_scenes::ARRIVAL_SUMMARY),
        Step::RestFor(Duration::from_millis(3500)),
    ],
};

pub(super) const TOAST_PROGRESS: Demo = Demo {
    name: "toast_progress",
    scene: &desktop_session_scenes::TOAST_PROGRESS,
    script: &[Step::RestFor(Duration::from_secs(5))],
};

pub(super) const TOAST_ACTIONS: Demo = Demo {
    name: "toast_actions",
    scene: &desktop_session_scenes::TOAST_ACTIONS,
    script: &[
        Step::HoverText("Later"),
        Step::HoverText("Decline"),
        Step::PressText("Accept"),
    ],
};

pub(super) const TOAST_WINDOW_ANCHORED: Demo = Demo {
    name: "toast_window_anchored",
    scene: &desktop_session_scenes::TOAST_WINDOW_ANCHORED,
    script: &[Step::HoverText(desktop_session_scenes::WINDOW_SUMMARY)],
};
