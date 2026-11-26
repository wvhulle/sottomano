use sottomano_widgets::group_menus::group_menu_state::GroupMenuTopic;

use crate::{
    fixtures::{
        app_launcher, appearance_presets, audio, caldav, claude_usage, gamepad, idle_lock,
        location, login_session, tailnet, unchanging, voice_dictation, wallpaper,
    },
    scenes::{GroupShown, Scene, SceneBackdrop},
};

pub static APP_LAUNCHER_FREQUENT: Scene = Scene {
    name: "app_launcher_frequent",
    group: |_| app_launcher::group(),
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Applications),
};

pub static APP_LAUNCHER_OVERFLOW: Scene = Scene {
    name: "app_launcher_overflow",
    group: |_| app_launcher::group(),
    live: |_| app_launcher::scrolled_item_list(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Applications),
};

pub static TAILNET_EXIT_NODE: Scene = Scene {
    name: "tailnet_exit_node",
    group: tailnet::group,
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Network),
};

pub static LOCATION_FIX: Scene = Scene {
    name: "location_fix",
    group: location::group,
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Agenda),
};

pub static CALDAV_AGENDA: Scene = Scene {
    name: "caldav_agenda",
    group: caldav::group,
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Agenda),
};

pub static AI_USAGE_WINDOWS: Scene = Scene {
    name: "ai_usage_windows",
    group: claude_usage::group,
    live: claude_usage::live,
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Services),
};

pub static IDLE_SCREEN_OFF: Scene = Scene {
    name: "idle_screen_off",
    group: idle_lock::group,
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Session),
};

pub static APPEARANCE_LOOKS: Scene = Scene {
    name: "appearance_looks",
    group: |_| appearance_presets::group(),
    live: |_| unchanging(),
    backdrop: SceneBackdrop::FrostablePhoto,
    shown: GroupShown::InMenu(GroupMenuTopic::Settings),
};

pub static GAMEPAD_CONTROLLER: Scene = Scene {
    name: "gamepad_controller",
    group: |_| gamepad::group(),
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Gaming),
};

pub static AUDIO_VOLUME: Scene = Scene {
    name: "audio_volume",
    group: audio::group,
    live: audio::live,
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::OnPanel,
};

pub static VOICE_DICTATION_MODEL: Scene = Scene {
    name: "voice_dictation_model",
    group: voice_dictation::group,
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Media),
};

pub static LOGIN_SESSION_KERNEL: Scene = Scene {
    name: "login_session_kernel",
    group: login_session::group,
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Session),
};

pub static WALLPAPER_ARTICLE: Scene = Scene {
    name: "wallpaper_article",
    group: wallpaper::group,
    live: |_| unchanging(),
    backdrop: SceneBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Display),
};
