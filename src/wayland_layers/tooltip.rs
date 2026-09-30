pub mod host_surfaces;
pub mod hover_area;
pub mod tooltip_content;
pub mod tooltip_transition;

use iced::{Rectangle, window::Id};

pub use crate::wayland_layers::tooltip::{
    host_surfaces::TooltipHost,
    hover_area::{hoverable, on_surface},
    tooltip_content::relative,
};
use crate::{data::source::weather::Allergen, wayland_layers::surface::Position};

#[derive(Debug, Clone, PartialEq)]
pub enum HoverTarget {
    Cpu,
    Memory,
    Temperature,
    Disk,
    Battery,
    PowerDraw,
    BatteryTime,
    CornerRadius,
    FloatOffset,
    BlurBackground,
    Xray,
    DropShadow,
    ShadowSoftness,
    ShadowOpacity,
    WindowCornerRadius,
    WindowGaps,
    DisconnectedGroups,
    AutoCollapseGroups,
    WorkspacesPlacement,
    TintChrome,
    Journal,
    JournalSystem,
    JournalUser,
    JournalKernel,
    Bluetooth,
    Ethernet,
    Wifi,
    Mobile,
    AudioSink,
    AudioSource,
    PowerProfile,
    Backlight(String),
    GammaDim,
    ColorTemperature,
    AutoColorTemperature,
    ClightAuto,
    DarkMode,
    DarkModeSchedule,
    SunriseOffset,
    SunsetOffset,
    IdleLockUser,
    IdleScreenOff,
    IdleStandby,
    IdlePowerDown,
    InternetDown,
    InternetUp,
    Tailscale,
    Wireguard(String),
    WeatherTemp,
    WeatherWind,
    WeatherRain,
    Pollen,
    Allergen(Allergen),
    Location,
    Wallpaper,
    WallpaperPlace,
    WallpaperArticle,
    WallpaperPhoto,
    Media,
    AiOpenRouter,
    AiClaudeOauth,
    AiClaudeCcusage,
    AiCopilotMetered,
    AiCopilotFree,
    AiCopilotCli,
    Tray(String),
    Window(u64),
    Workspace(u64),
    Output(String),
}

#[derive(Debug, Clone)]
pub enum TooltipMessage {
    EnterAnchor {
        target: HoverTarget,
        anchor: Rectangle,
    },
    Enter {
        surface: Id,
        position: Position,
        target: HoverTarget,
        anchor: Rectangle,
    },
    Exit {
        target: HoverTarget,
    },
    EnterTooltip,
    ExitTooltip,
    SwitchIntentElapsed {
        seq: u64,
    },
}
