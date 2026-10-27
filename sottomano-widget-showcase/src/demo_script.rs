use std::{borrow::Cow, time::Duration};

use sottomano_sources::login_session_presence::idle_session_actions::IdleAction;
use sottomano_widgets::{
    clock::Clock,
    hover_tooltips::{GroupHandle, HoverTarget},
    widget_groups::{
        audio_group::{AudioTarget, ControlKind},
        caldav_group::CaldavTarget,
        claude_usage_group::ClaudeUsageTarget,
        compute_group::ComputeTarget,
        gamepad_group::GamepadTarget,
        login_session_group::LoginSessionTarget,
        media_group::{MediaTarget, PlayerTarget},
        memory_board_group::MemoryBoardTarget,
        voice_dictation_group::VoiceDictationTarget,
        vpn_group::VpnTarget,
        wallpaper_group::WallpaperTarget,
        weather_group::WeatherTarget,
        window_group::WindowTarget,
    },
};

use crate::{
    fixtures::{
        audio, caldav, claude_usage, compute, gamepad, idle_lock, login_session, media,
        memory_board, niri_session, notification_toasts, unchanging, voice_dictation, vpn,
        wallpaper, weather,
    },
    showcase_message::ShowcaseMessage,
    showcased_group::ShowcasedGroup,
};

pub struct Demo {
    pub name: &'static str,
    pub group: fn(Clock) -> ShowcasedGroup,
    pub live: fn(Clock) -> Vec<ShowcaseMessage>,
    pub script: &'static [Step],
}

pub enum Step {
    Hover(HoverTarget),
    Press(HoverTarget),
    Drag { target: HoverTarget, by: f32 },
    HoverText(&'static str),
    PressText(&'static str),
    RestFor(Duration),
}

pub const DEMOS: &[Demo] = &[
    Demo {
        name: "compute_cpu",
        group: compute::group,
        live: compute::live,
        script: &[Step::Press(HoverTarget::Compute(ComputeTarget::CpuLoad))],
    },
    Demo {
        name: "compute_model",
        group: compute::group,
        live: compute::live,
        script: &[
            Step::Press(HoverTarget::Compute(ComputeTarget::CpuLoad)),
            Step::Hover(HoverTarget::Compute(ComputeTarget::CpuModel)),
        ],
    },
    Demo {
        name: "memory_board_memory",
        group: memory_board::group,
        live: memory_board::live,
        script: &[Step::Press(HoverTarget::MemoryBoard(
            MemoryBoardTarget::Memory,
        ))],
    },
    Demo {
        name: "memory_board_board",
        group: memory_board::group,
        live: memory_board::live,
        script: &[
            Step::Press(HoverTarget::MemoryBoard(MemoryBoardTarget::Memory)),
            Step::Hover(HoverTarget::MemoryBoard(MemoryBoardTarget::Board)),
        ],
    },
    Demo {
        name: "compute_temperature",
        group: compute::group,
        live: compute::live,
        script: &[
            Step::Press(HoverTarget::Compute(ComputeTarget::CpuLoad)),
            Step::Hover(HoverTarget::Compute(ComputeTarget::CpuTemperature)),
        ],
    },
    Demo {
        name: "vpn_tailscale",
        group: vpn::group,
        live: unchanging,
        script: &[
            Step::Press(HoverTarget::Vpn(VpnTarget::NetworkManagerVpn(
                Cow::Borrowed(vpn::COMPANY_VPN_NAME),
            ))),
            Step::Hover(HoverTarget::Vpn(VpnTarget::Tailscale)),
        ],
    },
    Demo {
        name: "weather_conditions",
        group: weather::group,
        live: unchanging,
        script: &[Step::Press(HoverTarget::Weather(
            WeatherTarget::Temperature,
        ))],
    },
    Demo {
        name: "weather_location",
        group: weather::group,
        live: unchanging,
        script: &[
            Step::Press(HoverTarget::Weather(WeatherTarget::Temperature)),
            Step::Hover(HoverTarget::Weather(WeatherTarget::Location)),
        ],
    },
    Demo {
        name: "caldav_agenda",
        group: caldav::group,
        live: unchanging,
        script: &[
            Step::Press(HoverTarget::Caldav(CaldavTarget::UpcomingEvents)),
            Step::Hover(HoverTarget::Caldav(CaldavTarget::UpcomingEvents)),
            Step::Hover(HoverTarget::Caldav(CaldavTarget::OpenTasks)),
        ],
    },
    Demo {
        name: "ai_usage_windows",
        group: claude_usage::group,
        live: claude_usage::live,
        script: &[
            Step::Press(HoverTarget::ClaudeUsage(ClaudeUsageTarget::UsageWindows)),
            Step::Hover(HoverTarget::ClaudeUsage(ClaudeUsageTarget::UsageWindows)),
        ],
    },
    Demo {
        name: "media_track",
        group: media::group,
        live: unchanging,
        script: &[
            Step::Press(HoverTarget::Media(PlayerTarget {
                bus: Cow::Borrowed(media::SPOTIFY_BUS),
                control: MediaTarget::Player,
            })),
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
        script: &[
            Step::Press(HoverTarget::IdleTimeout(IdleAction::Lock)),
            Step::Hover(HoverTarget::IdleTimeout(IdleAction::ScreenOff)),
        ],
    },
    Demo {
        name: "idle_lock",
        group: idle_lock::group,
        live: unchanging,
        script: &[
            Step::Press(HoverTarget::IdleTimeout(IdleAction::Lock)),
            Step::Hover(HoverTarget::IdleTimeout(IdleAction::Lock)),
        ],
    },
    Demo {
        name: "gamepad_controller",
        group: gamepad::group,
        live: unchanging,
        script: &[Step::Hover(HoverTarget::Gamepad(
            GamepadTarget::Controller {
                slot: gamepad::DUALSENSE_SLOT,
            },
        ))],
    },
    Demo {
        name: "audio_volume",
        group: audio::group,
        live: audio::live,
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
        name: "audio_microphone",
        group: audio::group,
        live: audio::live,
        script: &[
            Step::Press(HoverTarget::Audio(AudioTarget::Control(ControlKind::Sink))),
            Step::Hover(HoverTarget::Audio(AudioTarget::Control(
                ControlKind::Source,
            ))),
        ],
    },
    Demo {
        name: "voice_dictation_model",
        group: voice_dictation::group,
        live: unchanging,
        script: &[
            Step::Press(HoverTarget::VoiceDictation(VoiceDictationTarget::Dictation)),
            Step::Hover(HoverTarget::VoiceDictation(VoiceDictationTarget::Runtime)),
            Step::Hover(HoverTarget::VoiceDictation(
                VoiceDictationTarget::SpeechModel,
            )),
            Step::Hover(HoverTarget::VoiceDictation(
                VoiceDictationTarget::RuntimeConfiguration,
            )),
            Step::Hover(HoverTarget::VoiceDictation(
                VoiceDictationTarget::InferenceHardware,
            )),
        ],
    },
    Demo {
        name: "login_session_kernel",
        group: login_session::group,
        live: unchanging,
        script: &[
            Step::Press(HoverTarget::GroupHandle(GroupHandle::LoginSession)),
            Step::Hover(HoverTarget::LoginSession(LoginSessionTarget::Kernel)),
            Step::Hover(HoverTarget::LoginSession(LoginSessionTarget::Compositor)),
        ],
    },
    Demo {
        name: "wallpaper_place",
        group: wallpaper::group,
        live: unchanging,
        script: &[Step::Press(HoverTarget::Wallpaper(WallpaperTarget::Lookup))],
    },
    Demo {
        name: "wallpaper_article",
        group: wallpaper::group,
        live: unchanging,
        script: &[
            Step::Press(HoverTarget::Wallpaper(WallpaperTarget::Lookup)),
            Step::Hover(HoverTarget::Wallpaper(WallpaperTarget::Article)),
        ],
    },
    Demo {
        name: "bottom_workspaces",
        group: niri_session::bottom_bar,
        live: unchanging,
        script: &[Step::Hover(HoverTarget::Workspace(2))],
    },
    Demo {
        name: "bottom_window",
        group: niri_session::bottom_bar,
        live: unchanging,
        script: &[Step::Hover(HoverTarget::Window(WindowTarget::Pick(2)))],
    },
    Demo {
        name: "bottom_process_inspector",
        group: niri_session::bottom_bar,
        live: unchanging,
        script: &[Step::Hover(HoverTarget::Window(WindowTarget::Inspect(2)))],
    },
    Demo {
        name: "sidebar_workspaces",
        group: niri_session::workspace_sidebar,
        live: unchanging,
        script: &[
            Step::Hover(HoverTarget::Workspace(1)),
            Step::Hover(HoverTarget::Workspace(2)),
            Step::Hover(HoverTarget::Workspace(3)),
        ],
    },
    Demo {
        name: "toast_arrival",
        group: notification_toasts::arrival,
        live: unchanging,
        script: &[
            Step::HoverText(notification_toasts::ARRIVAL_SUMMARY),
            Step::RestFor(Duration::from_secs(5)),
        ],
    },
    Demo {
        name: "toast_progress",
        group: notification_toasts::progress,
        live: notification_toasts::downloading,
        script: &[Step::RestFor(Duration::from_secs(10))],
    },
    Demo {
        name: "toast_actions",
        group: notification_toasts::actions,
        live: unchanging,
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
        script: &[Step::HoverText(notification_toasts::WINDOW_SUMMARY)],
    },
];
