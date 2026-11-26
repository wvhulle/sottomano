use std::{borrow::Cow, time::Duration};

use sottomano_widget_scenes::{
    scenes::{indicator_group_scenes, menu_tour_scenes},
    script_steps::{Step, indicator_row, instance_row},
};
use sottomano_widgets::{
    group_indicators::indicator_group_identity::{
        IndicatorGroupFamily, IndicatorGroupIdentity, IndicatorGroupInstance,
    },
    group_menus::group_menu_state::GroupMenuTopic,
    hover_cards::HoverTarget,
};

use crate::demo_script::Demo;

pub(super) const APP_LAUNCHER_FREQUENT: Demo = Demo {
    name: "app_launcher_frequent",
    scene: &indicator_group_scenes::APP_LAUNCHER_FREQUENT,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Applications)),
        Step::PressText("Frequently launched on this workspace"),
        Step::HoverText("Firefox"),
    ],
};

pub(super) const APP_LAUNCHER_OVERFLOW: Demo = Demo {
    name: "app_launcher_overflow",
    scene: &indicator_group_scenes::APP_LAUNCHER_OVERFLOW,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Applications)),
        Step::PressText("Utilities"),
        Step::HoverText("Utilities"),
    ],
};

pub(super) const TAILNET_EXIT_NODE: Demo = Demo {
    name: "tailnet_exit_node",
    scene: &indicator_group_scenes::TAILNET_EXIT_NODE,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Network)),
        Step::PressText("Tailscale"),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::Tailnet,
            "Exit via home-server",
        )),
    ],
};

pub(super) const LOCATION_FIX: Demo = Demo {
    name: "location_fix",
    scene: &indicator_group_scenes::LOCATION_FIX,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Agenda)),
        Step::PressText("Location"),
        Step::Hover(indicator_row(IndicatorGroupFamily::Location, "Estimate")),
    ],
};

pub(super) const WEATHER_CONDITIONS: Demo = Demo {
    name: "weather_conditions",
    scene: &menu_tour_scenes::WEATHER_IN_AGENDA,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Agenda)),
        Step::PressText("Weather"),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::Weather,
            "Temperature forecast",
        )),
    ],
};

pub(super) const CALDAV_AGENDA: Demo = Demo {
    name: "caldav_agenda",
    scene: &indicator_group_scenes::CALDAV_AGENDA,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Agenda)),
        Step::PressText("Calendar"),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::Caldav,
            "Upcoming events",
        )),
        Step::Hover(indicator_row(IndicatorGroupFamily::Caldav, "Open tasks")),
    ],
};

pub(super) const AI_USAGE_WINDOWS: Demo = Demo {
    name: "ai_usage_windows",
    scene: &indicator_group_scenes::AI_USAGE_WINDOWS,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Services)),
        Step::PressText("Claude usage"),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::ClaudeUsage,
            "Subscription windows",
        )),
    ],
};

pub(super) const IDLE_SCREEN_OFF: Demo = Demo {
    name: "idle_screen_off",
    scene: &indicator_group_scenes::IDLE_SCREEN_OFF,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Session)),
        Step::PressText("Idle"),
        Step::Hover(instance_row(
            IndicatorGroupIdentity {
                family: IndicatorGroupFamily::IdleTimeouts,
                instance: IndicatorGroupInstance::Durable(Cow::Borrowed("chain")),
            },
            "Timeline",
        )),
    ],
};

pub(super) const APPEARANCE_LOOKS: Demo = Demo {
    name: "appearance_looks",
    scene: &indicator_group_scenes::APPEARANCE_LOOKS,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Behaviour)),
        Step::PressText("Looks"),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::AppearancePresets,
            "Frosted dark",
        )),
        Step::PressText("Frosted dark"),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::AppearancePresets,
            "Clear dark",
        )),
        Step::RestFor(Duration::from_millis(600)),
    ],
};

pub(super) const GAMEPAD_CONTROLLER: Demo = Demo {
    name: "gamepad_controller",
    scene: &indicator_group_scenes::GAMEPAD_CONTROLLER,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Gaming)),
        Step::PressText("DualSense Wireless Controller"),
        Step::Hover(instance_row(
            IndicatorGroupIdentity {
                family: IndicatorGroupFamily::Gamepad,
                instance: IndicatorGroupInstance::Durable(Cow::Borrowed("d4:2f:4b:69:6b:2f")),
            },
            "Game controller",
        )),
    ],
};

pub(super) const AUDIO_VOLUME: Demo = Demo {
    name: "audio_volume",
    scene: &indicator_group_scenes::AUDIO_VOLUME,
    script: &[
        Step::PressFace(IndicatorGroupIdentity::sole(IndicatorGroupFamily::Audio)),
        Step::PressFace(IndicatorGroupIdentity::sole(IndicatorGroupFamily::Audio)),
        Step::DragFace {
            group: IndicatorGroupIdentity::sole(IndicatorGroupFamily::Audio),
            by: -24.0,
        },
    ],
};

pub(super) const VOICE_DICTATION_MODEL: Demo = Demo {
    name: "voice_dictation_model",
    scene: &indicator_group_scenes::VOICE_DICTATION_MODEL,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Media)),
        Step::PressText("Voice dictation"),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::VoiceDictation,
            "Speech model",
        )),
    ],
};

pub(super) const LOGIN_SESSION_KERNEL: Demo = Demo {
    name: "login_session_kernel",
    scene: &indicator_group_scenes::LOGIN_SESSION_KERNEL,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Session)),
        Step::PressText("Login session"),
        Step::Hover(indicator_row(IndicatorGroupFamily::LoginSession, "Kernel")),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::LoginSession,
            "Compositor",
        )),
    ],
};

pub(super) const WALLPAPER_ARTICLE: Demo = Demo {
    name: "wallpaper_article",
    scene: &indicator_group_scenes::WALLPAPER_ARTICLE,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Display)),
        Step::PressText("Wallpaper"),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::Wallpaper,
            "Wallpaper lookup",
        )),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::Wallpaper,
            "Wallpaper article",
        )),
    ],
};
