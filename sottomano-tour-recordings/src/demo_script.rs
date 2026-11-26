use sottomano_widget_scenes::{scenes::Scene, script_steps::Step};

#[cfg(test)]
mod demo_script_tests;
mod desktop_session_demos;
mod group_menu_demos;
mod guest_machine_demos;
mod guided_tour_demos;
mod indicator_group_demos;

pub struct Demo {
    pub name: &'static str,
    pub scene: &'static Scene,
    pub script: &'static [Step],
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
