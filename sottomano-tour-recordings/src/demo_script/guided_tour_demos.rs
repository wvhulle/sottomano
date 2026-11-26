use std::{borrow::Cow, time::Duration};

use sottomano_widget_scenes::{
    scenes::menu_tour_scenes,
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

pub(super) const TOUR_HARDWARE: Demo = Demo {
    name: "tour_hardware",
    scene: &menu_tour_scenes::HARDWARE_MENU,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Hardware)),
        Step::PressText("Compute"),
        Step::Hover(indicator_row(IndicatorGroupFamily::Compute, "CPU load")),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::Compute,
            "CPU temperature",
        )),
        Step::PressText("Memory"),
        Step::Hover(indicator_row(IndicatorGroupFamily::MemoryBoard, "RAM")),
    ],
};

pub(super) const TOUR_NETWORK: Demo = Demo {
    name: "tour_network",
    scene: &menu_tour_scenes::TOUR_NETWORK,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Network)),
        Step::PressText("Tailscale"),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::Tailnet,
            "Reachable from",
        )),
        Step::Press(indicator_row(
            IndicatorGroupFamily::Tailnet,
            "Exit via home-server",
        )),
        Step::Press(indicator_row(
            IndicatorGroupFamily::Tailnet,
            "Exit via home-server",
        )),
        Step::PressText("WireGuard"),
        Step::Press(indicator_row(
            IndicatorGroupFamily::WireguardTunnels,
            "wg-lab",
        )),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::WireguardTunnels,
            "wg-lab",
        )),
    ],
};

pub(super) const TOUR_AGENDA: Demo = Demo {
    name: "tour_agenda",
    scene: &menu_tour_scenes::TOUR_AGENDA,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Agenda)),
        Step::PressText("Calendar"),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::Caldav,
            "Upcoming events",
        )),
        Step::PressText("Weather"),
        Step::Hover(indicator_row(IndicatorGroupFamily::Weather, "Rain")),
        Step::PressText("Location"),
        Step::Hover(indicator_row(IndicatorGroupFamily::Location, "Estimate")),
    ],
};

pub(super) const TOUR_DISPLAY: Demo = Demo {
    name: "tour_display",
    scene: &menu_tour_scenes::TOUR_DISPLAY,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Display)),
        Step::PressText("Dark mode"),
        Step::Press(indicator_row(
            IndicatorGroupFamily::DarkMode,
            "Color scheme",
        )),
        Step::RestFor(Duration::from_millis(1500)),
        Step::Press(indicator_row(
            IndicatorGroupFamily::DarkMode,
            "Color scheme",
        )),
        Step::RestFor(Duration::from_millis(1200)),
    ],
};

pub(super) const TOUR_SERVICES: Demo = Demo {
    name: "tour_services",
    scene: &menu_tour_scenes::TOUR_SERVICES,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Services)),
        Step::PressText("3 CI runs"),
        Step::Hover(indicator_row(IndicatorGroupFamily::CiRuns, "website #185")),
        Step::PressText("Claude usage"),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::ClaudeUsage,
            "Subscription windows",
        )),
    ],
};

pub(super) const TOUR_SESSION: Demo = Demo {
    name: "tour_session",
    scene: &menu_tour_scenes::TOUR_SESSION,
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
        Step::PressText("Login session"),
        Step::Hover(indicator_row(IndicatorGroupFamily::LoginSession, "Kernel")),
    ],
};

pub(super) const SESSION_FOCUS_TIMELINE: Demo = Demo {
    name: "session_focus_timeline",
    scene: &menu_tour_scenes::FOCUS_TIMELINE,
    script: &[
        Step::PressFace(IndicatorGroupIdentity::sole(
            IndicatorGroupFamily::SessionFocus,
        )),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::SessionFocus,
            "Timeline",
        )),
    ],
};

pub(super) const TOUR_MEDIA: Demo = Demo {
    name: "tour_media",
    scene: &menu_tour_scenes::MEDIA_MENU,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Media)),
        Step::PressText("Spotify"),
        Step::Hover(instance_row(
            IndicatorGroupIdentity {
                family: IndicatorGroupFamily::Media,
                instance: IndicatorGroupInstance::Session {
                    durable: Cow::Borrowed("Spotify"),
                    session: Cow::Borrowed("org.mpris.MediaPlayer2.spotify"),
                },
            },
            "Track",
        )),
        Step::PressText("Audio"),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::Audio,
            "Microphone volume",
        )),
        Step::PressText("Voice dictation"),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::VoiceDictation,
            "Speech model",
        )),
    ],
};

pub(super) const TOUR_VIRTUALIZATION: Demo = Demo {
    name: "tour_virtualization",
    scene: &menu_tour_scenes::TOUR_VIRTUALIZATION,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
        Step::PressText("Podman containers"),
        Step::Press(indicator_row(
            IndicatorGroupFamily::PodmanEngines,
            "fedora-toolbox-42",
        )),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::PodmanEngines,
            "fedora-toolbox-42",
        )),
        Step::PressText("Incus instances"),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::IncusInstances,
            "build-box",
        )),
        Step::PressText("Docker containers"),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::DockerEngines,
            "postgres as root",
        )),
    ],
};

const STARDEW_VALLEY_GROUP: IndicatorGroupIdentity = IndicatorGroupIdentity {
    family: IndicatorGroupFamily::SteamGame,
    instance: IndicatorGroupInstance::Durable(Cow::Borrowed("413150")),
};

pub(super) const TOUR_GAMING: Demo = Demo {
    name: "tour_gaming",
    scene: &menu_tour_scenes::TOUR_GAMING,
    script: &[
        Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Gaming)),
        Step::PressText("Steam downloads"),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::SteamDownloads,
            "Hades II",
        )),
        Step::PressText("Stardew Valley"),
        Step::Hover(instance_row(STARDEW_VALLEY_GROUP, "ProtonDB")),
        Step::Hover(instance_row(STARDEW_VALLEY_GROUP, "Install size")),
        Step::Hover(instance_row(STARDEW_VALLEY_GROUP, "Launch")),
    ],
};

pub(super) const MENU_BAR_INLINE: Demo = Demo {
    name: "menu_bar_inline",
    scene: &menu_tour_scenes::OFFICE_MENU_BAR,
    script: &[
        Step::Press(HoverTarget::WindowTopMenu(menu_tour_scenes::FILE_MENU_ID)),
        Step::HoverText("Recent Documents"),
        Step::HoverText("budget-2027.ods"),
        Step::Hover(HoverTarget::WindowTopMenu(menu_tour_scenes::FORMAT_MENU_ID)),
        Step::HoverText("Text"),
        Step::HoverText("Italic"),
    ],
};
