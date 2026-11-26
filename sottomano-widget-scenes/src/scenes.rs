use corredo::popovers::NestedMenuPresentation;
use iced::Size;
use sottomano_widgets::{
    clock::Clock, group_indicators::indicator_group_identity::IndicatorGroupIdentity,
    group_menus::group_menu_state::GroupMenuTopic,
};

use crate::{showcase_message::ShowcaseMessage, showcased_group::ShowcasedGroup};

pub const CANVAS: Size = Size::new(1280.0, 1080.0);
pub const PHOTO_CANVAS: Size = Size::new(1000.0, 320.0);

pub mod desktop_session_scenes;
pub mod group_menu_scenes;
pub mod guest_machine_scenes;
pub mod indicator_group_scenes;
pub mod menu_tour_scenes;
#[cfg(test)]
mod scene_tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupShown {
    OnPanel,
    InMenu(GroupMenuTopic),
    InMenuPresented {
        menu: GroupMenuTopic,
        presentation: NestedMenuPresentation,
    },
    PinnedFrom {
        menu: GroupMenuTopic,
        group: IndicatorGroupIdentity,
    },
}

pub struct Scene {
    pub name: &'static str,
    pub group: fn(Clock) -> ShowcasedGroup,
    pub live: fn(Clock) -> Vec<ShowcaseMessage>,
    pub shown: GroupShown,
    pub backdrop: SceneBackdrop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneBackdrop {
    Transparent,
    FrostablePhoto,
}

impl SceneBackdrop {
    #[must_use]
    pub fn canvas(self) -> Size {
        match self {
            SceneBackdrop::Transparent => CANVAS,
            SceneBackdrop::FrostablePhoto => PHOTO_CANVAS,
        }
    }
}

pub const SCENES: &[&Scene] = &[
    &menu_tour_scenes::HARDWARE_MENU,
    &menu_tour_scenes::TOUR_NETWORK,
    &menu_tour_scenes::WEATHER_IN_AGENDA,
    &menu_tour_scenes::FOCUS_TIMELINE,
    &menu_tour_scenes::MEDIA_MENU,
    &menu_tour_scenes::TOUR_VIRTUALIZATION,
    &indicator_group_scenes::APP_LAUNCHER_FREQUENT,
    &indicator_group_scenes::APP_LAUNCHER_OVERFLOW,
    &indicator_group_scenes::TAILNET_EXIT_NODE,
    &indicator_group_scenes::LOCATION_FIX,
    &guest_machine_scenes::MICROVM_GUESTS_CLOUD_HYPERVISOR,
    &guest_machine_scenes::LXC_CONTAINERS_RUNNING,
    &guest_machine_scenes::FLATPAK_SANDBOXES_APP,
    &guest_machine_scenes::WAYDROID_CONTAINER_SESSION,
    &guest_machine_scenes::VIRTUALBOX_MACHINES_HEADLESS,
    &guest_machine_scenes::VMWARE_MACHINES_WORKSTATION,
    &guest_machine_scenes::APPTAINER_CONTAINERS_IMAGE,
    &guest_machine_scenes::CGROUP_MEMORY_LIMITS_CI_RUNNERS,
    &guest_machine_scenes::CI_RUNNER_TASKS,
    &indicator_group_scenes::CALDAV_AGENDA,
    &indicator_group_scenes::AI_USAGE_WINDOWS,
    &indicator_group_scenes::IDLE_SCREEN_OFF,
    &indicator_group_scenes::APPEARANCE_LOOKS,
    &indicator_group_scenes::GAMEPAD_CONTROLLER,
    &indicator_group_scenes::AUDIO_VOLUME,
    &indicator_group_scenes::VOICE_DICTATION_MODEL,
    &indicator_group_scenes::LOGIN_SESSION_KERNEL,
    &indicator_group_scenes::WALLPAPER_ARTICLE,
    &desktop_session_scenes::BOTTOM_BAR,
    &desktop_session_scenes::BOTTOM_WINDOW_DIRECTORY_GROUPS,
    &desktop_session_scenes::TOAST_ARRIVAL,
    &desktop_session_scenes::TOAST_PROGRESS,
    &desktop_session_scenes::TOAST_ACTIONS,
    &desktop_session_scenes::TOAST_WINDOW_ANCHORED,
    &group_menu_scenes::GROUP_MENU_ACCORDION_SEVERAL,
    &group_menu_scenes::GROUP_MENU_APPEARANCE,
    &group_menu_scenes::COMPUTE_CPU_PINNED,
];
