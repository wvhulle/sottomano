pub mod core_grid;
pub mod hover_area;
pub mod tooltip_content;
pub mod tooltip_surfaces;
pub mod tooltip_transition;

use iced::{Rectangle, window::Id};
use sottomano_config::screen_edges::Position;
use sottomano_sources::login_session_presence::idle_session_actions::IdleAction;

pub use crate::hover_tooltips::{hover_area::hoverable, tooltip_surfaces::TooltipHost};
use crate::{
    widget_groups::{
        audio_group::AudioTarget,
        bluetooth_adapters_group::BluetoothTarget,
        caldav_group::CaldavTarget,
        claude_usage_group::ClaudeUsageTarget,
        clock_group::ClockTarget,
        color_temperature_group::ColorTemperatureTarget,
        compute_group::ComputeTarget,
        copilot_usage_group::CopilotUsageTarget,
        dark_mode_group::DarkModeTarget,
        disk_group::DiskTarget,
        ethernet_ports_group::EthernetPortTarget,
        gamepad_group::GamepadTarget,
        gpu_group::GpuTarget,
        journal_events_group::JournalTarget,
        llama_server_group::LlamaServerTarget,
        login_session_group::LoginSessionTarget,
        media_group::PlayerTarget,
        memory_board_group::MemoryBoardTarget,
        monitor_group::MonitorTarget,
        net_interfaces_group::NetInterfaceTarget,
        niri_window_group::NiriWindowTarget,
        notification_group::NotificationTarget,
        openrouter_credit_group::OpenRouterCreditTarget,
        overlay_settings_group::OverlaySettingsTarget,
        panel_settings_group::PanelSettingsTarget,
        pending_upgrades_group::PendingUpgradesTarget,
        power_group::PowerTarget,
        quiet_schedule_group::QuietScheduleTarget,
        reachability_group::ReachabilityTarget,
        session_focus_group::SessionFocusTarget,
        session_menu::SessionTarget,
        sidebar_settings_group::SidebarSettingsTarget,
        systemd_manager_group::SystemdTarget,
        voice_dictation_group::VoiceDictationTarget,
        vpn_group::VpnTarget,
        wallpaper_group::WallpaperTarget,
        weather_group::WeatherTarget,
        window_group::{WindowGroupMessage, WindowTarget},
    },
    widgets::phosphor::{Phosphor, Weight},
};

#[derive(Debug, Clone, PartialEq)]
pub enum HoverTarget {
    Compute(ComputeTarget),
    MemoryBoard(MemoryBoardTarget),
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
    EthernetPort(EthernetPortTarget),
    Bluetooth(BluetoothTarget),
    Audio(AudioTarget),
    Monitor(MonitorTarget),
    ColorTemperature(ColorTemperatureTarget),
    DarkMode(DarkModeTarget),
    VoiceDictation(VoiceDictationTarget),
    IdleTimeout(IdleAction),
    Reachability(ReachabilityTarget),
    Vpn(VpnTarget),
    Weather(WeatherTarget),
    Wallpaper(WallpaperTarget),
    Caldav(CaldavTarget),
    Media(PlayerTarget),
    Gamepad(GamepadTarget),
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
    GroupHandle {
        handle: GroupHandle,
        group_name: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupHandle {
    Audio,
    Caldav,
    ClaudeUsage,
    ColorTemperature,
    CopilotUsage,
    DarkMode,
    IdleTimeouts,
    Reachability,
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
    SettingsDrawer,
    SidebarSettings,
    Systemd,
    Tray,
    VoiceDictation,
    Vpn,
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
    SwitchIntentElapsed,
}

#[derive(Debug, Clone)]
pub enum PopupMessage {
    Tooltip(TooltipMessage),
    CopyText(String),
    OpenUrl(String),
    FocusWindow(u64),
    RefreshHoveredReading,
    WindowGroup(WindowGroupMessage),
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
