use sottomano_widgets::group_menus::group_menu_state::GroupMenuTopic;

use crate::{
    fixtures::{audio, group_menu_members, session_focus, unchanging, weather},
    scenes::{GroupShown, Scene, SceneBackdrop},
};

pub static HARDWARE_MENU: Scene = Scene {
    name: "hardware_menu",
    group: group_menu_members::hardware_group_menu,
    live: group_menu_members::hardware_menu_live,
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
