use std::time::Duration;

use sottomano_sources::login_session_presence::idle_session_actions::IdleAction;
use sottomano_widgets::{
    clock::Clock,
    hover_tooltips::HoverTarget,
    shell_message::Message,
    widget_groups::{
        audio_group::{AudioTarget, ControlKind},
        claude_usage_group::ClaudeUsageTarget,
        compute_group::ComputeTarget,
        gamepad_group::GamepadTarget,
        media_group::MediaTarget,
        memory_board_group::MemoryBoardTarget,
        wallpaper_group::WallpaperTarget,
        weather_group::WeatherTarget,
        window_group::WindowTarget,
    },
};

use crate::{
    fixtures::{
        audio, claude_usage, compute, gamepad, idle_lock, media, memory_board, niri_session,
        notification_toasts, unchanging, wallpaper, weather,
    },
    showcased_group::ShowcasedGroup,
};

pub struct Demo {
    pub name: &'static str,
    pub group: fn(Clock) -> ShowcasedGroup,
    pub live: fn(Clock) -> Vec<Message>,
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
        script: &[Step::Press(HoverTarget::Compute(ComputeTarget::Cpu))],
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
        name: "compute_temperature",
        group: compute::group,
        live: compute::live,
        script: &[
            Step::Press(HoverTarget::Compute(ComputeTarget::Cpu)),
            Step::Hover(HoverTarget::Compute(ComputeTarget::Temperature)),
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
        script: &[Step::Hover(HoverTarget::Media(MediaTarget::Track))],
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
