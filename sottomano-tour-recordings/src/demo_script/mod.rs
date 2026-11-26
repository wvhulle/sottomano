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
    guided_tour_demos::TOUR_AGENDA,
    guided_tour_demos::TOUR_DISPLAY,
    guided_tour_demos::TOUR_MEDIA,
    guided_tour_demos::TOUR_VIRTUALIZATION,
    guided_tour_demos::TOUR_SERVICES,
    guided_tour_demos::TOUR_SESSION,
    guided_tour_demos::TOUR_GAMING,
    guided_tour_demos::MENU_BAR_INLINE,
    guided_tour_demos::SESSION_FOCUS_TIMELINE,
    indicator_group_demos::APP_LAUNCHER_FREQUENT,
    indicator_group_demos::APP_LAUNCHER_OVERFLOW,
    indicator_group_demos::TAILNET_EXIT_NODE,
    indicator_group_demos::WEATHER_CONDITIONS,
    indicator_group_demos::LOCATION_ESTIMATE,
    indicator_group_demos::CALENDAR_EVENTS,
    indicator_group_demos::CLAUDE_USAGE_WINDOWS,
    indicator_group_demos::IDLE_TIMELINE,
    indicator_group_demos::APPEARANCE_LOOKS,
    indicator_group_demos::GAMEPAD_BINDINGS,
    indicator_group_demos::AUDIO_SPEAKER_VOLUME,
    indicator_group_demos::VOICE_DICTATION_MODEL,
    indicator_group_demos::LOGIN_SESSION_KERNEL,
    indicator_group_demos::WALLPAPER_ARTICLE,
    guest_machine_demos::WAYDROID_CONTAINER_SESSION,
    guest_machine_demos::MEMORY_LIMITS_SLICE,
    guest_machine_demos::CI_RUNS_TASK,
    desktop_session_demos::BOTTOM_WORKSPACES,
    desktop_session_demos::BOTTOM_WINDOW_DIRECTORY_GROUPS,
    desktop_session_demos::BOTTOM_PROCESS_INSPECTOR,
    desktop_session_demos::TOAST_ARRIVAL,
    desktop_session_demos::TOAST_PROGRESS,
    desktop_session_demos::TOAST_ACTIONS,
    desktop_session_demos::TOAST_WINDOW_ANCHORED,
    group_menu_demos::GROUP_MENU_ACCORDION_SEVERAL,
    group_menu_demos::GROUP_MENU_PIN,
    group_menu_demos::GROUP_MENU_SETTINGS,
    group_menu_demos::COMPUTE_CPU_PINNED,
    group_menu_demos::MEMORY_GROWTH_ALERT,
];
