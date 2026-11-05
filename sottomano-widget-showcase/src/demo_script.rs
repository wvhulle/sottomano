use std::{borrow::Cow, time::Duration};

use sottomano_sources::login_session_presence::idle_session_actions::IdleAction;
use sottomano_widgets::{
    clock::Clock,
    group_menus::{
        group_menu_presentation::{
            GroupMenuLayout, GroupMenuOpenTrigger, GroupMenuPresentation, OpenGroupLimit,
        },
        group_menu_state::{GroupMenuPin, GroupMenuTopic},
    },
    hover_cards::HoverTarget,
    widget_groups::{
        appearance_presets_group::AppearancePresetsTarget,
        audio_group::{AudioTarget, ControlKind},
        caldav_group::CaldavTarget,
        claude_usage_group::ClaudeUsageTarget,
        compute_group::ComputeTarget,
        gamepad_group::GamepadTarget,
        login_session_group::LoginSessionTarget,
        media_group::{MediaTarget, PlayerTarget},
        memory_board_group::MemoryBoardTarget,
        tailnet_group::TailnetTarget,
        voice_dictation_group::VoiceDictationTarget,
        vpn_group::VpnTarget,
        wallpaper_group::WallpaperTarget,
        weather_group::WeatherTarget,
        widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity},
        window_group::WindowTarget,
        wireguard_server_group::WireguardServerTarget,
    },
};

use crate::{
    fixtures::{
        app_launcher, appearance_presets, audio, caldav, claude_usage, compute, gamepad,
        group_menu_members, idle_lock, login_session, media, memory_board, niri_session,
        notification_toasts, shell_settings, tailnet, unchanging, voice_dictation, vpn, wallpaper,
        weather, wireguard_server,
    },
    showcase_message::ShowcaseMessage,
    showcased_group::ShowcasedGroup,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupShown {
    OnPanel,
    InMenu(GroupMenuTopic),
    InMenuPresented {
        menu: GroupMenuTopic,
        presentation: GroupMenuPresentation,
    },
    PinnedFrom {
        menu: GroupMenuTopic,
        group: WidgetGroupIdentity,
    },
}

pub struct Demo {
    pub name: &'static str,
    pub group: fn(Clock) -> ShowcasedGroup,
    pub live: fn(Clock) -> Vec<ShowcaseMessage>,
    pub shown: GroupShown,
    pub script: &'static [Step],
}

pub enum Step {
    Hover(HoverTarget),
    Press(HoverTarget),
    PressPin(GroupMenuPin),
    Drag { target: HoverTarget, by: f32 },
    HoverText(&'static str),
    HoverApp(&'static str),
    PressText(&'static str),
    RestFor(Duration),
}

pub const DEMOS: &[Demo] = &[
    Demo {
        name: "app_launcher_frequent",
        group: app_launcher::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Applications),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Applications)),
            Step::PressText("Frequently launched on this workspace"),
            Step::HoverApp("firefox"),
        ],
    },
    Demo {
        name: "app_launcher_overflow",
        group: app_launcher::group,
        live: app_launcher::scrolled_item_list,
        shown: GroupShown::InMenu(GroupMenuTopic::Applications),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Applications)),
            Step::PressText("Utilities"),
            Step::HoverText("Utilities"),
        ],
    },
    Demo {
        name: "compute_cpu",
        group: compute::group,
        live: compute::live,
        shown: GroupShown::InMenu(GroupMenuTopic::Hardware),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Hardware)),
            Step::PressText("Compute"),
            Step::Hover(HoverTarget::Compute(ComputeTarget::CpuLoad)),
        ],
    },
    Demo {
        name: "compute_model",
        group: compute::group,
        live: compute::live,
        shown: GroupShown::InMenu(GroupMenuTopic::Hardware),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Hardware)),
            Step::PressText("Compute"),
            Step::Hover(HoverTarget::Compute(ComputeTarget::CpuModel)),
        ],
    },
    Demo {
        name: "memory_board_memory",
        group: memory_board::group,
        live: memory_board::live,
        shown: GroupShown::InMenu(GroupMenuTopic::Hardware),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Hardware)),
            Step::PressText("Memory"),
            Step::Hover(HoverTarget::MemoryBoard(MemoryBoardTarget::Memory)),
        ],
    },
    Demo {
        name: "vpn_tailscale",
        group: vpn::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Network),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Network)),
            Step::PressText("VPN"),
            Step::Hover(HoverTarget::Vpn(VpnTarget::TailscaleExitNode(
                Cow::Borrowed(vpn::HOME_EXIT_NODE_IP),
            ))),
        ],
    },
    Demo {
        name: "wireguard_server",
        group: wireguard_server::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Network),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Network)),
            Step::PressText("WireGuard server"),
            Step::Hover(HoverTarget::WireguardServer(WireguardServerTarget {
                interface: Cow::Borrowed(wireguard_server::HOME_SERVER_TUNNEL_NAME),
            })),
        ],
    },
    Demo {
        name: "tailnet_access",
        group: tailnet::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Network),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Network)),
            Step::PressText("Tailscale"),
            Step::Hover(HoverTarget::Tailnet(TailnetTarget::InboundAccess)),
        ],
    },
    Demo {
        name: "weather_conditions",
        group: weather::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Agenda),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Agenda)),
            Step::PressText("Weather"),
            Step::Hover(HoverTarget::Weather(WeatherTarget::Temperature)),
        ],
    },
    Demo {
        name: "weather_precipitation",
        group: weather::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Agenda),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Agenda)),
            Step::PressText("Weather"),
            Step::Hover(HoverTarget::Weather(WeatherTarget::Rain)),
        ],
    },
    Demo {
        name: "caldav_agenda",
        group: caldav::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Agenda),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Agenda)),
            Step::PressText("Calendar"),
            Step::Hover(HoverTarget::Caldav(CaldavTarget::UpcomingEvents)),
            Step::Hover(HoverTarget::Caldav(CaldavTarget::OpenTasks)),
        ],
    },
    Demo {
        name: "ai_usage_windows",
        group: claude_usage::group,
        live: claude_usage::live,
        shown: GroupShown::InMenu(GroupMenuTopic::Services),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Services)),
            Step::PressText("Claude usage"),
            Step::Hover(HoverTarget::ClaudeUsage(ClaudeUsageTarget::UsageWindows)),
        ],
    },
    Demo {
        name: "media_track",
        group: media::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Media),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Media)),
            Step::PressText("Spotify"),
            Step::Hover(HoverTarget::Media(PlayerTarget {
                bus: Cow::Borrowed(media::SPOTIFY_BUS),
                control: MediaTarget::Track,
            })),
        ],
    },
    Demo {
        name: "idle_screen_off",
        group: idle_lock::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Session),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Session)),
            Step::PressText("Screen off"),
            Step::Hover(HoverTarget::IdleTimeout(IdleAction::ScreenOff)),
        ],
    },
    Demo {
        name: "appearance_looks",
        group: appearance_presets::group,
        live: unchanging,
        shown: GroupShown::OnPanel,
        script: &[
            Step::Hover(HoverTarget::AppearancePresets(
                AppearancePresetsTarget::Menu,
            )),
            Step::PressText("Clear dark"),
            Step::RestFor(Duration::from_millis(1200)),
        ],
    },
    Demo {
        name: "gamepad_controller",
        group: gamepad::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Media),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Media)),
            Step::PressText("DualSense Wireless Controller"),
            Step::Hover(HoverTarget::Gamepad(GamepadTarget::Controller {
                slot: gamepad::DUALSENSE_SLOT,
            })),
        ],
    },
    Demo {
        name: "audio_volume",
        group: audio::group,
        live: audio::live,
        shown: GroupShown::OnPanel,
        script: &[
            Step::Press(HoverTarget::Audio(AudioTarget::Control(ControlKind::Sink))),
            Step::Press(HoverTarget::Audio(AudioTarget::Control(ControlKind::Sink))),
            Step::Drag {
                target: HoverTarget::Audio(AudioTarget::Control(ControlKind::Sink)),
                by: -24.0,
            },
        ],
    },
    Demo {
        name: "voice_dictation_model",
        group: voice_dictation::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Media),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Media)),
            Step::PressText("Voice dictation"),
            Step::Hover(HoverTarget::VoiceDictation(
                VoiceDictationTarget::SpeechModel,
            )),
        ],
    },
    Demo {
        name: "login_session_kernel",
        group: login_session::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Session),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Session)),
            Step::PressText("Login session"),
            Step::Hover(HoverTarget::LoginSession(LoginSessionTarget::Kernel)),
            Step::Hover(HoverTarget::LoginSession(LoginSessionTarget::Compositor)),
        ],
    },
    Demo {
        name: "wallpaper_article",
        group: wallpaper::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Display),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Display)),
            Step::PressText("Wallpaper"),
            Step::Hover(HoverTarget::Wallpaper(WallpaperTarget::Lookup)),
            Step::Hover(HoverTarget::Wallpaper(WallpaperTarget::Article)),
        ],
    },
    Demo {
        name: "bottom_workspaces",
        group: niri_session::bottom_bar,
        live: unchanging,
        shown: GroupShown::OnPanel,
        script: &[Step::Hover(HoverTarget::Workspace(2))],
    },
    Demo {
        name: "bottom_window_directory_groups",
        group: niri_session::terminals_grouped_by_directory,
        live: unchanging,
        shown: GroupShown::OnPanel,
        script: &[Step::Hover(HoverTarget::Window(WindowTarget::Pick(2)))],
    },
    Demo {
        name: "bottom_process_inspector",
        group: niri_session::bottom_bar,
        live: unchanging,
        shown: GroupShown::OnPanel,
        script: &[Step::Hover(HoverTarget::Window(WindowTarget::Inspect(2)))],
    },
    Demo {
        name: "toast_arrival",
        group: notification_toasts::arrival,
        live: unchanging,
        shown: GroupShown::OnPanel,
        script: &[
            Step::HoverText(notification_toasts::ARRIVAL_SUMMARY),
            Step::RestFor(Duration::from_millis(3500)),
        ],
    },
    Demo {
        name: "toast_progress",
        group: notification_toasts::progress,
        live: notification_toasts::downloading,
        shown: GroupShown::OnPanel,
        script: &[Step::RestFor(Duration::from_secs(5))],
    },
    Demo {
        name: "toast_actions",
        group: notification_toasts::actions,
        live: unchanging,
        shown: GroupShown::OnPanel,
        script: &[
            Step::HoverText("Later"),
            Step::HoverText("Decline"),
            Step::PressText("Accept"),
        ],
    },
    Demo {
        name: "toast_window_anchored",
        group: notification_toasts::next_to_window,
        live: unchanging,
        shown: GroupShown::OnPanel,
        script: &[Step::HoverText(notification_toasts::WINDOW_SUMMARY)],
    },
    Demo {
        name: "group_menu_accordion_several",
        group: group_menu_members::hardware_group_menu,
        live: group_menu_members::hardware_menu_live,
        shown: GroupShown::InMenuPresented {
            menu: GroupMenuTopic::Hardware,
            presentation: GroupMenuPresentation {
                layout: GroupMenuLayout::Accordion,
                open_trigger: GroupMenuOpenTrigger::Press,
                accordion_open_limit: OpenGroupLimit::Several,
            },
        },
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Hardware)),
            Step::PressText("Compute"),
            Step::PressText("Memory"),
        ],
    },
    Demo {
        name: "group_menu_media",
        group: group_menu_members::media_group_menu,
        live: audio::live,
        shown: GroupShown::InMenu(GroupMenuTopic::Media),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Media)),
            Step::PressText("Spotify"),
            Step::PressText("Audio"),
            Step::Hover(HoverTarget::Audio(AudioTarget::Control(
                ControlKind::Source,
            ))),
        ],
    },
    Demo {
        name: "group_menu_pin",
        group: group_menu_members::hardware_group_menu,
        live: group_menu_members::hardware_menu_live,
        shown: GroupShown::InMenu(GroupMenuTopic::Hardware),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Hardware)),
            Step::PressPin(GroupMenuPin::Group(WidgetGroupIdentity::sole(
                WidgetGroupFamily::MemoryBoard,
            ))),
            Step::RestFor(Duration::from_millis(1500)),
        ],
    },
    Demo {
        name: "group_menu_appearance",
        group: shell_settings::sharing_a_unit,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Appearance),
        script: &[Step::Press(HoverTarget::GroupMenu(
            GroupMenuTopic::Appearance,
        ))],
    },
    Demo {
        name: "compute_cpu_pinned",
        group: compute::group,
        live: compute::live,
        shown: GroupShown::PinnedFrom {
            menu: GroupMenuTopic::Hardware,
            group: WidgetGroupIdentity::sole(WidgetGroupFamily::Compute),
        },
        script: &[
            Step::Hover(HoverTarget::Compute(ComputeTarget::CpuLoad)),
            Step::Press(HoverTarget::Compute(ComputeTarget::CpuLoad)),
        ],
    },
];
