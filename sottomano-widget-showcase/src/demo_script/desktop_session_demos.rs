use std::time::Duration;

use sottomano_widgets::{hover_cards::HoverTarget, indicator_groups::window_group::WindowTarget};

use crate::{
    demo_script::{Demo, DemoBackdrop, GroupShown, Step, UiPart},
    fixtures::{desktop_session, notification_toasts, unchanging},
};

pub(super) const BOTTOM_WORKSPACES: Demo = Demo {
    name: "bottom_workspaces",
    parts: &[UiPart::HoverCards, UiPart::Workspaces],
    group: |_| desktop_session::bottom_bar(),
    live: |_| unchanging(),
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::OnPanel,
    script: &[Step::Hover(HoverTarget::Workspace(2))],
};

pub(super) const BOTTOM_WINDOW_DIRECTORY_GROUPS: Demo = Demo {
    name: "bottom_window_directory_groups",
    parts: &[UiPart::HoverCards, UiPart::WindowList],
    group: |_| desktop_session::terminals_grouped_by_directory(),
    live: |_| unchanging(),
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::OnPanel,
    script: &[Step::Hover(HoverTarget::Window(WindowTarget::Usage(2)))],
};

pub(super) const BOTTOM_PROCESS_INSPECTOR: Demo = Demo {
    name: "bottom_process_inspector",
    parts: &[UiPart::HoverCards, UiPart::WindowList],
    group: |_| desktop_session::bottom_bar(),
    live: |_| unchanging(),
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::OnPanel,
    script: &[Step::Hover(HoverTarget::Window(WindowTarget::Inspect(2)))],
};

pub(super) const TOAST_ARRIVAL: Demo = Demo {
    name: "toast_arrival",
    parts: &[UiPart::Toasts],
    group: |_| notification_toasts::arrival(),
    live: |_| unchanging(),
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::OnPanel,
    script: &[
        Step::HoverText(notification_toasts::ARRIVAL_SUMMARY),
        Step::RestFor(Duration::from_millis(3500)),
    ],
};

pub(super) const TOAST_PROGRESS: Demo = Demo {
    name: "toast_progress",
    parts: &[UiPart::Toasts],
    group: |_| notification_toasts::progress(),
    live: notification_toasts::downloading,
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::OnPanel,
    script: &[Step::RestFor(Duration::from_secs(5))],
};

pub(super) const TOAST_ACTIONS: Demo = Demo {
    name: "toast_actions",
    parts: &[UiPart::Toasts],
    group: |_| notification_toasts::actions(),
    live: |_| unchanging(),
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::OnPanel,
    script: &[
        Step::HoverText("Later"),
        Step::HoverText("Decline"),
        Step::PressText("Accept"),
    ],
};

pub(super) const TOAST_WINDOW_ANCHORED: Demo = Demo {
    name: "toast_window_anchored",
    parts: &[UiPart::Toasts],
    group: |_| notification_toasts::next_to_window(),
    live: |_| unchanging(),
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::OnPanel,
    script: &[Step::HoverText(notification_toasts::WINDOW_SUMMARY)],
};
