use std::{borrow::Cow, time::Duration};

use corredo::popovers::NestedMenuPresentation;
use iced::Size;
use sottomano_widgets::{
    clock::Clock,
    group_indicators::indicator_group_identity::{IndicatorGroupFamily, IndicatorGroupIdentity},
    group_menus::group_menu_state::{GroupMenuPin, GroupMenuTopic},
    hover_cards::{HoverTarget, IndicatorKey},
};

use crate::{
    animation_recording::{CANVAS, PHOTO_CANVAS},
    showcase_message::ShowcaseMessage,
    showcased_group::ShowcasedGroup,
};

#[cfg(test)]
mod demo_script_tests;
mod desktop_session_demos;
mod group_menu_demos;
mod guest_machine_demos;
mod guided_tour_demos;
mod indicator_group_demos;
mod ui_parts;

pub use ui_parts::UiPart;

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

pub struct Demo {
    pub name: &'static str,
    pub parts: &'static [UiPart],
    pub group: fn(Clock) -> ShowcasedGroup,
    pub live: fn(Clock) -> Vec<ShowcaseMessage>,
    pub shown: GroupShown,
    pub script: &'static [Step],
    pub backdrop: DemoBackdrop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoBackdrop {
    Transparent,
    FrostablePhoto,
}

impl DemoBackdrop {
    #[must_use]
    pub fn canvas(self) -> Size {
        match self {
            DemoBackdrop::Transparent => CANVAS,
            DemoBackdrop::FrostablePhoto => PHOTO_CANVAS,
        }
    }
}

pub enum Step {
    Hover(HoverTarget),
    Press(HoverTarget),
    HoverFace(IndicatorGroupIdentity),
    PressFace(IndicatorGroupIdentity),
    PressPin(GroupMenuPin),
    DragFace {
        group: IndicatorGroupIdentity,
        by: f32,
    },
    HoverText(&'static str),
    PressText(&'static str),
    RestFor(Duration),
}

const fn instance_row(group: IndicatorGroupIdentity, row_name: &'static str) -> HoverTarget {
    HoverTarget::Indicator(IndicatorKey {
        group,
        name: Cow::Borrowed(row_name),
    })
}

const fn indicator_row(family: IndicatorGroupFamily, row_name: &'static str) -> HoverTarget {
    instance_row(IndicatorGroupIdentity::sole(family), row_name)
}

pub const DEMOS: &[Demo] = &[
    guided_tour_demos::TOUR_HARDWARE,
    guided_tour_demos::TOUR_NETWORK,
    guided_tour_demos::TOUR_WEATHER,
    guided_tour_demos::FOCUS_TIMELINE,
    guided_tour_demos::TOUR_MEDIA,
    guided_tour_demos::TOUR_VIRTUALIZATION,
    indicator_group_demos::APP_LAUNCHER_FREQUENT,
    indicator_group_demos::APP_LAUNCHER_OVERFLOW,
    indicator_group_demos::TAILNET_EXIT_NODE,
    indicator_group_demos::WEATHER_CONDITIONS,
    indicator_group_demos::LOCATION_FIX,
    guest_machine_demos::MICROVM_GUESTS_CLOUD_HYPERVISOR,
    guest_machine_demos::LXC_CONTAINERS_RUNNING,
    guest_machine_demos::FLATPAK_SANDBOXES_APP,
    guest_machine_demos::WAYDROID_CONTAINER_SESSION,
    guest_machine_demos::VIRTUALBOX_MACHINES_HEADLESS,
    guest_machine_demos::VMWARE_MACHINES_WORKSTATION,
    guest_machine_demos::APPTAINER_CONTAINERS_IMAGE,
    guest_machine_demos::CGROUP_MEMORY_LIMITS_CI_RUNNERS,
    guest_machine_demos::CI_RUNNER_TASKS,
    indicator_group_demos::CALDAV_AGENDA,
    indicator_group_demos::AI_USAGE_WINDOWS,
    indicator_group_demos::IDLE_SCREEN_OFF,
    indicator_group_demos::APPEARANCE_LOOKS,
    indicator_group_demos::GAMEPAD_CONTROLLER,
    indicator_group_demos::AUDIO_VOLUME,
    indicator_group_demos::VOICE_DICTATION_MODEL,
    indicator_group_demos::LOGIN_SESSION_KERNEL,
    indicator_group_demos::WALLPAPER_ARTICLE,
    desktop_session_demos::BOTTOM_WORKSPACES,
    desktop_session_demos::BOTTOM_WINDOW_DIRECTORY_GROUPS,
    desktop_session_demos::BOTTOM_PROCESS_INSPECTOR,
    desktop_session_demos::TOAST_ARRIVAL,
    desktop_session_demos::TOAST_PROGRESS,
    desktop_session_demos::TOAST_ACTIONS,
    desktop_session_demos::TOAST_WINDOW_ANCHORED,
    group_menu_demos::GROUP_MENU_ACCORDION_SEVERAL,
    group_menu_demos::GROUP_MENU_MEDIA,
    group_menu_demos::GROUP_MENU_PIN,
    group_menu_demos::GROUP_MENU_APPEARANCE,
    group_menu_demos::COMPUTE_CPU_PINNED,
];
