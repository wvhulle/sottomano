pub mod host_surfaces;
pub mod hover_area;
pub mod tooltip_connector;
pub mod tooltip_content;
pub mod tooltip_placement;
pub mod tooltip_transition;

use iced::{Rectangle, window::Id};

use crate::data::source::Tooltip;
pub use crate::wayland_layers::tooltip::{
    host_surfaces::TooltipHost,
    hover_area::{hoverable, on_surface},
    tooltip_content::relative,
};

#[derive(Debug, Clone, PartialEq)]
pub enum HoverTarget {
    Cpu,
    Memory,
    Temperature,
    Disk,
    Battery,
    PowerDraw,
    CornerRadius,
    FloatOffset,
    BlurBackground,
    DisconnectedGroups,
    TintChrome,
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
    IdleLockUser,
    IdleScreenOff,
    IdleStandby,
    IdlePowerDown,
    InternetDown,
    InternetUp,
    Tailscale,
    Wireguard(String),
    WeatherTemp,
    WeatherRain,
    PollenBirch,
    PollenGrass,
    Location,
    Media,
    AiOpenRouter,
    AiClaudeOauth,
    AiClaudeCcusage,
    AiCopilotMetered,
    AiCopilotFree,
    AiCopilotCli,
    Tray(Box<Tooltip>),
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
        target: HoverTarget,
        anchor: Rectangle,
    },
    Exit {
        target: HoverTarget,
    },
    EnterTooltip,
    ExitTooltip,
}
