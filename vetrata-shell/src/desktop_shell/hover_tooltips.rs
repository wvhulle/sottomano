pub mod core_grid;
pub mod hover_area;
pub mod tooltip_content;
pub mod tooltip_surfaces;
pub mod tooltip_transition;
pub mod tooltip_updates;

use iced::{Rectangle, window::Id};

pub use crate::desktop_shell::hover_tooltips::{
    hover_area::{hoverable, on_surface, pressable_card},
    tooltip_content::{ongoing_for, relative},
    tooltip_surfaces::TooltipHost,
};
use crate::{
    desktop_shell::monitor_surfaces::Position,
    widget_groups::{
        audio_group::AudioTarget, claude_usage_group::ClaudeUsageTarget, clock_group::ClockTarget,
        color_temperature_group::ColorTemperatureTarget, compute_group::ComputeTarget,
        copilot_usage_group::CopilotUsageTarget, dark_mode_group::DarkModeTarget,
        disk_group::DiskTarget, gpu_group::GpuTarget, idle_timeouts_group::TimerKind,
        internet_group::InternetTarget, journal_events_group::JournalTarget,
        llama_server_group::LlamaServerTarget, login_session_group::LoginSessionTarget,
        media_group::MediaTarget, monitor_group::MonitorTarget,
        net_interfaces_group::NetInterfaceTarget, niri_window_group::NiriWindowTarget,
        notification_group::NotificationTarget, openrouter_credit_group::OpenRouterCreditTarget,
        overlay_settings_group::OverlaySettingsTarget, panel_settings_group::PanelSettingsTarget,
        pending_upgrades_group::PendingUpgradesTarget, power_group::PowerTarget,
        quiet_schedule_group::QuietScheduleTarget, session_focus_group::SessionFocusTarget,
        session_menu::SessionTarget, sidebar_settings_group::SidebarSettingsTarget,
        systemd_manager_group::SystemdTarget, wallpaper_group::WallpaperTarget,
        weather_group::WeatherTarget, window_group::WindowTarget,
    },
    widgets::phosphor::{Phosphor, Weight},
};

#[derive(Debug, Clone, PartialEq)]
pub enum HoverTarget {
    Compute(ComputeTarget),
    Gpu(GpuTarget),
    Disk(DiskTarget),
    Power(PowerTarget),
    PanelSettings(PanelSettingsTarget),
    SidebarSettings(SidebarSettingsTarget),
    OverlaySettings(OverlaySettingsTarget),
    NiriWindow(NiriWindowTarget),
    Systemd(SystemdTarget),
    Journal(JournalTarget),
    PendingUpgrades(PendingUpgradesTarget),
    NetInterface(NetInterfaceTarget),
    Audio(AudioTarget),
    Monitor(MonitorTarget),
    ColorTemperature(ColorTemperatureTarget),
    DarkMode(DarkModeTarget),
    IdleTimeout(TimerKind),
    Internet(InternetTarget),
    Weather(WeatherTarget),
    Wallpaper(WallpaperTarget),
    Media(MediaTarget),
    QuickLaunch(String),
    ClaudeUsage(ClaudeUsageTarget),
    CopilotUsage(CopilotUsageTarget),
    OpenRouterCredit(OpenRouterCreditTarget),
    LlamaServer(LlamaServerTarget),
    SessionFocus(SessionFocusTarget),
    LoginSession(LoginSessionTarget),
    Clock(ClockTarget),
    Notification(NotificationTarget),
    QuietSchedule(QuietScheduleTarget),
    Session(SessionTarget),
    Tray(String),
    Window(WindowTarget),
    Workspace(u64),
    WorkspaceOrder,
    GroupHandle(GroupHandle),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupHandle {
    Audio,
    ClaudeUsage,
    ColorTemperature,
    CopilotUsage,
    DarkMode,
    IdleTimeouts,
    Internet,
    LlamaServer,
    LoginSession,
    Monitor,
    NetInterfaces,
    NiriWindow,
    OpenRouterCredit,
    OverlaySettings,
    PanelSettings,
    Power,
    QuickLaunch,
    QuietSchedule,
    SidebarSettings,
    Systemd,
    Tray,
}

impl PressHints {
    #[must_use]
    pub fn group_handle(expanded: bool, scroll: Option<&'static str>) -> Self {
        PressHints {
            left: Some(if expanded {
                "collapse the group"
            } else {
                "expand the group"
            }),
            scroll,
            ..PressHints::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseGesture {
    LeftClick,
    MiddleClick,
    RightClick,
    Scroll,
}

impl MouseGesture {
    pub const ALL: [MouseGesture; 4] = [
        MouseGesture::LeftClick,
        MouseGesture::MiddleClick,
        MouseGesture::RightClick,
        MouseGesture::Scroll,
    ];

    #[must_use]
    pub fn glyph(self) -> Phosphor {
        match self {
            MouseGesture::LeftClick => Phosphor::MouseLeftClick(Weight::Regular),
            MouseGesture::MiddleClick => Phosphor::MouseMiddleClick(Weight::Regular),
            MouseGesture::RightClick => Phosphor::MouseRightClick(Weight::Regular),
            MouseGesture::Scroll => Phosphor::MouseScroll(Weight::Regular),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PressHints {
    pub left: Option<&'static str>,
    pub middle: Option<&'static str>,
    pub right: Option<&'static str>,
    pub scroll: Option<&'static str>,
}

impl PressHints {
    #[must_use]
    pub fn does(self, gesture: MouseGesture) -> Option<&'static str> {
        match gesture {
            MouseGesture::LeftClick => self.left,
            MouseGesture::MiddleClick => self.middle,
            MouseGesture::RightClick => self.right,
            MouseGesture::Scroll => self.scroll,
        }
    }

    #[must_use]
    pub fn is_empty(self) -> bool {
        MouseGesture::ALL
            .into_iter()
            .all(|gesture| self.does(gesture).is_none())
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
    PressHintsChanged {
        target: HoverTarget,
        press_hints: PressHints,
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
            MouseGesture::ALL.map(|gesture| hints.does(gesture)),
            [
                Some("focus the window"),
                None,
                Some("disconnect the tunnel"),
                None,
            ]
        );
        assert!(!hints.is_empty());
        assert!(PressHints::default().is_empty());
    }
}
