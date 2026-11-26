use sottomano_widgets::group_menus::group_menu_state::GroupMenuTopic;

pub use crate::fixtures::window_menu_bar::{FILE_MENU_ID, FORMAT_MENU_ID};
use crate::{
    fixtures::{
        audio, claude_usage, group_menu_members, session_focus, steam_client, unchanging, weather,
        window_menu_bar,
    },
    scenes::{GroupShown, Scene, SceneBackdrop},
};

pub static HARDWARE_MENU: Scene = Scene {
    name: "hardware_menu",
    group: group_menu_members::hardware_group_menu,
    live: group_menu_members::hardware_menu_live,
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Hardware),
};

pub static HARDWARE_MENU_RUNAWAY_MEMORY: Scene = Scene {
    name: "hardware_menu_runaway_memory",
    group: group_menu_members::hardware_group_menu,
    live: group_menu_members::hardware_menu_runaway_memory_live,
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Hardware),
};

pub static TOUR_NETWORK: Scene = Scene {
    name: "tour_network",
    group: group_menu_members::network_group_menu,
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Network),
};

pub static WEATHER_IN_AGENDA: Scene = Scene {
    name: "weather_in_agenda",
    group: weather::group,
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Agenda),
};

pub static FOCUS_TIMELINE: Scene = Scene {
    name: "focus_timeline",
    group: session_focus::group,
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::OnPanel,
};

pub static MEDIA_MENU: Scene = Scene {
    name: "media_menu",
    group: group_menu_members::media_group_menu,
    live: audio::live,
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Media),
};

pub static TOUR_VIRTUALIZATION: Scene = Scene {
    name: "tour_virtualization",
    group: group_menu_members::virtualization_group_menu,
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
};

pub static TOUR_AGENDA: Scene = Scene {
    name: "tour_agenda",
    group: group_menu_members::agenda_group_menu,
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Agenda),
};

pub static TOUR_DISPLAY: Scene = Scene {
    name: "tour_display",
    group: group_menu_members::display_group_menu,
    live: |_| unchanging(),
    backdrop: SceneBackdrop::FrostablePhoto,
    shown: GroupShown::InMenu(GroupMenuTopic::Display),
};

pub static TOUR_SERVICES: Scene = Scene {
    name: "tour_services",
    group: group_menu_members::services_group_menu,
    live: claude_usage::live,
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Services),
};

pub static TOUR_SESSION: Scene = Scene {
    name: "tour_session",
    group: group_menu_members::session_group_menu,
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Session),
};

pub static TOUR_GAMING: Scene = Scene {
    name: "tour_gaming",
    group: steam_client::group,
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Gaming),
};

pub static OFFICE_MENU_BAR: Scene = Scene {
    name: "office_menu_bar",
    group: |_| window_menu_bar::group(),
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::OnPanel,
};
