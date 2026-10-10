pub mod hover_area;
pub mod tooltip_content;
pub mod tooltip_surfaces;
pub mod tooltip_transition;
pub mod tooltip_updates;

use chrono_tz::Tz;
use iced::{Rectangle, window::Id};

pub use crate::desktop_shell::hover_tooltips::{
    hover_area::{hoverable, on_surface},
    tooltip_content::relative,
    tooltip_surfaces::TooltipHost,
};
use crate::{
    data::source::weather::Allergen,
    desktop_shell::monitor_surfaces::Position,
    widgets::phosphor::{Phosphor, Weight},
};

#[derive(Debug, Clone, PartialEq)]
pub enum HoverTarget {
    Cpu,
    CpuClock,
    Memory,
    Temperature,
    Disk,
    DiskRead,
    DiskWrite,
    DriveHealth,
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
    PanelOpacity,
    OverlayOpacity,
    OverlayCornerRadius,
    ToastSpatialLayout,
    SystemdBootPhases,
    SystemdStartupUnits,
    SystemdFailedSystemUnits,
    SystemdFailedUserUnits,
    FailedSystemUnit(String),
    FailedUserUnit(String),
    SystemdSlowSystemJobs,
    SystemdSlowUserJobs,
    SlowSystemJob(String),
    SlowUserJob(String),
    SystemdLastShutdown,
    SystemdCoredumps,
    SystemdOomKills,
    JournalEvents,
    JournalKernel,
    JournalServiceErrors,
    PendingUpgrades,
    FirmwareUpgrades,
    Bluetooth,
    Ethernet,
    Wifi,
    Mobile,
    AudioSink,
    AudioSource,
    PowerProfile,
    CpuTuning,
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
    InternetTotal,
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
    AiClaudeUsageWindows,
    AiClaudeEnterpriseSpend,
    AiClaudeCcusagePeriods,
    AiClaudeCcusageBlock,
    AiCopilotMetered,
    AiCopilotFree,
    AiCopilotCli,
    LocalTime,
    LocalDate,
    ZoneTime(Tz),
    Tray(String),
    Window(u64),
    Workspace(u64),
    NotificationDoNotDisturb,
    NotificationSound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseGesture {
    LeftClick,
    MiddleClick,
    RightClick,
    Scroll,
}

impl MouseGesture {
    pub fn glyph(self) -> Phosphor {
        match self {
            MouseGesture::LeftClick => Phosphor::MouseLeftClick(Weight::Regular),
            MouseGesture::MiddleClick => Phosphor::MouseMiddleClick(Weight::Regular),
            MouseGesture::RightClick => Phosphor::MouseRightClick(Weight::Regular),
            MouseGesture::Scroll => Phosphor::MouseScroll(Weight::Regular),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GestureHint {
    pub gesture: MouseGesture,
    pub does: &'static str,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PressHints {
    pub left: Option<&'static str>,
    pub middle: Option<&'static str>,
    pub right: Option<&'static str>,
    pub scroll: Option<&'static str>,
}

impl PressHints {
    pub fn gestures(self) -> Vec<GestureHint> {
        [
            (MouseGesture::LeftClick, self.left),
            (MouseGesture::MiddleClick, self.middle),
            (MouseGesture::RightClick, self.right),
            (MouseGesture::Scroll, self.scroll),
        ]
        .into_iter()
        .filter_map(|(gesture, does)| does.map(|does| GestureHint { gesture, does }))
        .collect()
    }
}

#[derive(Debug, Clone)]
pub enum TooltipMessage {
    EnterAnchor {
        target: HoverTarget,
        anchor: Rectangle,
        press_hints: PressHints,
    },
    Enter {
        surface: Id,
        position: Position,
        target: HoverTarget,
        anchor: Rectangle,
        press_hints: PressHints,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn press_hints_list_only_gestures_that_do_something() {
        let hints = PressHints {
            left: Some("focus the window"),
            right: Some("disconnect the tunnel"),
            ..PressHints::default()
        };
        assert_eq!(
            hints.gestures(),
            [
                GestureHint {
                    gesture: MouseGesture::LeftClick,
                    does: "focus the window",
                },
                GestureHint {
                    gesture: MouseGesture::RightClick,
                    does: "disconnect the tunnel",
                },
            ]
        );
        assert!(PressHints::default().gestures().is_empty());
    }
}
