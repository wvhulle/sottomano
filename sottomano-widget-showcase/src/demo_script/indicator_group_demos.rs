use std::{borrow::Cow, time::Duration};

use sottomano_widgets::{
    group_menus::group_menu_state::GroupMenuTopic,
    hover_cards::HoverTarget,
    indicator_groups::indicator_group_identity::{
        IndicatorGroupFamily, IndicatorGroupIdentity, IndicatorGroupInstance,
    },
};

use crate::{
    demo_script::{Demo, DemoBackdrop, GroupShown, Step, indicator_row, instance_row},
    fixtures::{
        app_launcher, appearance_presets, audio, caldav, claude_usage, gamepad, idle_lock,
        login_session, tailnet, unchanging, voice_dictation, wallpaper, weather,
    },
};

pub(super) const APP_LAUNCHER_FREQUENT: Demo = Demo {
    name: "app_launcher_frequent",
    group: |_| app_launcher::group(),
    live: |_| unchanging(),
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Applications),
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Applications)),
        Step::PressText("Frequently launched on this workspace"),
        Step::HoverText("Firefox"),
    ],
};

pub(super) const APP_LAUNCHER_OVERFLOW: Demo = Demo {
    name: "app_launcher_overflow",
    group: |_| app_launcher::group(),
    live: |_| app_launcher::scrolled_item_list(),
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Applications),
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Applications)),
        Step::PressText("Utilities"),
        Step::HoverText("Utilities"),
    ],
};

pub(super) const TAILNET_EXIT_NODE: Demo = Demo {
    name: "tailnet_exit_node",
    group: tailnet::group,
    live: |_| unchanging(),
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Network),
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Network)),
        Step::PressText("Tailscale"),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::Tailnet,
            "Exit via home-server",
        )),
    ],
};

pub(super) const WEATHER_CONDITIONS: Demo = Demo {
    name: "weather_conditions",
    group: weather::group,
    live: |_| unchanging(),
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Agenda),
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
    group: caldav::group,
    live: |_| unchanging(),
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Agenda),
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
    group: claude_usage::group,
    live: claude_usage::live,
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Services),
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
    group: idle_lock::group,
    live: |_| unchanging(),
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Session),
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
    group: |_| appearance_presets::group(),
    live: |_| unchanging(),
    backdrop: DemoBackdrop::FrostablePhoto,
    shown: GroupShown::InMenu(GroupMenuTopic::Behaviour),
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
    group: |_| gamepad::group(),
    live: |_| unchanging(),
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Gaming),
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
    group: audio::group,
    live: audio::live,
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::OnPanel,
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
    group: voice_dictation::group,
    live: |_| unchanging(),
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Media),
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
    group: login_session::group,
    live: |_| unchanging(),
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Session),
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
    group: wallpaper::group,
    live: |_| unchanging(),
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::InMenu(GroupMenuTopic::Display),
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
