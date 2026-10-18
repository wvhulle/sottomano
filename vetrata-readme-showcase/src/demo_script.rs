use vetrata_widgets::{
    hover_tooltips::{GroupHandle, HoverTarget},
    shell_message::Message,
    widget_groups::{
        audio_group::{AudioTarget, ControlKind},
        claude_usage_group::ClaudeUsageTarget,
        compute_group::ComputeTarget,
        idle_timeouts_group::TimerKind,
        media_group::MediaTarget,
        wallpaper_group::WallpaperTarget,
        weather_group::WeatherTarget,
        window_group::WindowTarget,
    },
};

use crate::{
    fixtures::{
        audio, claude_usage, compute, idle_lock, media, niri_session, unchanging, wallpaper,
        weather,
    },
    showcased_group::ShowcasedGroup,
};

pub struct Demo {
    pub name: &'static str,
    pub group: fn() -> ShowcasedGroup,
    pub live: fn(u32) -> Vec<Message>,
    pub script: &'static [Step],
}

pub enum Step {
    Hover(HoverTarget),
    Press(HoverTarget),
    Drag { target: HoverTarget, by: f32 },
}

pub const DEMOS: &[Demo] = &[
    Demo {
        name: "compute-cpu",
        group: compute::group,
        live: compute::live,
        script: &[Step::Press(HoverTarget::Compute(ComputeTarget::Cpu))],
    },
    Demo {
        name: "compute-memory",
        group: compute::group,
        live: compute::live,
        script: &[
            Step::Press(HoverTarget::Compute(ComputeTarget::Cpu)),
            Step::Hover(HoverTarget::Compute(ComputeTarget::Memory)),
        ],
    },
    Demo {
        name: "compute-temperature",
        group: compute::group,
        live: compute::live,
        script: &[
            Step::Press(HoverTarget::Compute(ComputeTarget::Cpu)),
            Step::Hover(HoverTarget::Compute(ComputeTarget::Temperature)),
        ],
    },
    Demo {
        name: "weather-conditions",
        group: weather::group,
        live: unchanging,
        script: &[Step::Press(HoverTarget::Weather(WeatherTarget::Temperature))],
    },
    Demo {
        name: "weather-location",
        group: weather::group,
        live: unchanging,
        script: &[
            Step::Press(HoverTarget::Weather(WeatherTarget::Temperature)),
            Step::Hover(HoverTarget::Weather(WeatherTarget::Location)),
        ],
    },
    Demo {
        name: "ai-usage-windows",
        group: claude_usage::group,
        live: claude_usage::live,
        script: &[
            Step::Press(HoverTarget::GroupHandle(GroupHandle::ClaudeUsage)),
            Step::Hover(HoverTarget::ClaudeUsage(ClaudeUsageTarget::UsageWindows)),
        ],
    },
    Demo {
        name: "media-track",
        group: media::group,
        live: media::live,
        script: &[Step::Hover(HoverTarget::Media(MediaTarget::Track))],
    },
    Demo {
        name: "idle-screen-off",
        group: idle_lock::group,
        live: unchanging,
        script: &[
            Step::Press(HoverTarget::GroupHandle(GroupHandle::IdleTimeouts)),
            Step::Hover(HoverTarget::IdleTimeout(TimerKind::ScreenOff)),
        ],
    },
    Demo {
        name: "idle-lock",
        group: idle_lock::group,
        live: unchanging,
        script: &[
            Step::Press(HoverTarget::GroupHandle(GroupHandle::IdleTimeouts)),
            Step::Hover(HoverTarget::IdleTimeout(TimerKind::LockUser)),
        ],
    },
    Demo {
        name: "audio-volume",
        group: audio::group,
        live: audio::live,
        script: &[
            Step::Press(HoverTarget::GroupHandle(GroupHandle::Audio)),
            Step::Press(HoverTarget::Audio(AudioTarget::Control(ControlKind::Sink))),
            Step::Drag {
                target: HoverTarget::Audio(AudioTarget::Control(ControlKind::Sink)),
                by: -24.0,
            },
        ],
    },
    Demo {
        name: "audio-microphone",
        group: audio::group,
        live: audio::live,
        script: &[
            Step::Press(HoverTarget::GroupHandle(GroupHandle::Audio)),
            Step::Hover(HoverTarget::Audio(AudioTarget::Control(
                ControlKind::Source,
            ))),
        ],
    },
    Demo {
        name: "wallpaper-place",
        group: wallpaper::group,
        live: unchanging,
        script: &[Step::Press(HoverTarget::Wallpaper(WallpaperTarget::Lookup))],
    },
    Demo {
        name: "wallpaper-article",
        group: wallpaper::group,
        live: unchanging,
        script: &[
            Step::Press(HoverTarget::Wallpaper(WallpaperTarget::Lookup)),
            Step::Hover(HoverTarget::Wallpaper(WallpaperTarget::Article)),
        ],
    },
    Demo {
        name: "bottom-workspaces",
        group: niri_session::bottom_bar,
        live: unchanging,
        script: &[Step::Hover(HoverTarget::Workspace(2))],
    },
    Demo {
        name: "bottom-window",
        group: niri_session::bottom_bar,
        live: unchanging,
        script: &[Step::Hover(HoverTarget::Window(WindowTarget::Pick(2)))],
    },
    Demo {
        name: "bottom-process-inspector",
        group: niri_session::bottom_bar,
        live: unchanging,
        script: &[Step::Hover(HoverTarget::Window(WindowTarget::Inspect(2)))],
    },
    Demo {
        name: "sidebar-workspaces",
        group: niri_session::workspace_sidebar,
        live: unchanging,
        script: &[
            Step::Hover(HoverTarget::Workspace(1)),
            Step::Hover(HoverTarget::Workspace(2)),
            Step::Hover(HoverTarget::Workspace(3)),
        ],
    },
];
