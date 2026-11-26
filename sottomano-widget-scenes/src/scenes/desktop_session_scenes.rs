pub use crate::fixtures::notification_toasts::{ARRIVAL_SUMMARY, WINDOW_SUMMARY};
use crate::{
    fixtures::{desktop_session, notification_toasts, unchanging},
    scenes::{GroupShown, Scene, SceneBackdrop},
};

pub static BOTTOM_BAR: Scene = Scene {
    name: "bottom_bar",
    group: |_| desktop_session::bottom_bar(),
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::OnPanel,
};

pub static BOTTOM_WINDOW_DIRECTORY_GROUPS: Scene = Scene {
    name: "bottom_window_directory_groups",
    group: |_| desktop_session::terminals_grouped_by_directory(),
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::OnPanel,
};

pub static TOAST_ARRIVAL: Scene = Scene {
    name: "toast_arrival",
    group: |_| notification_toasts::arrival(),
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::OnPanel,
};

pub static TOAST_PROGRESS: Scene = Scene {
    name: "toast_progress",
    group: |_| notification_toasts::progress(),
    live: notification_toasts::downloading,
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::OnPanel,
};

pub static TOAST_ACTIONS: Scene = Scene {
    name: "toast_actions",
    group: |_| notification_toasts::actions(),
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::OnPanel,
};

pub static TOAST_WINDOW_ANCHORED: Scene = Scene {
    name: "toast_window_anchored",
    group: |_| notification_toasts::next_to_window(),
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::OnPanel,
};
