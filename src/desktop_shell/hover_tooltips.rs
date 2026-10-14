pub mod hover_area;
pub mod tooltip_content;
pub mod tooltip_surfaces;
pub mod tooltip_transition;
pub mod tooltip_updates;

use iced::{Rectangle, window::Id};

pub use crate::desktop_shell::hover_tooltips::{
    hover_area::{hoverable, on_surface},
    tooltip_content::{ongoing_for, relative},
    tooltip_surfaces::TooltipHost,
};
use crate::{
    desktop_shell::monitor_surfaces::Position,
    widget_groups::{
        ai_group::AiTarget, audio_group::ControlKind, clock_group::ClockTarget,
        color_temperature_group::ColorTemperatureTarget, compute_group::ComputeTarget,
        dark_mode_group::DarkModeTarget, disk_group::DiskTarget, focus_time_group::FocusTimeTarget,
        gpu_group::GpuTarget, idle_timeouts_group::TimerKind, internet_group::InternetTarget,
        journal_events_group::JournalTarget, monitor_group::MonitorTarget,
        net_interfaces_group::NetInterfaceTarget, niri_window_group::NiriWindowTarget,
        notification_group::NotificationTarget, overlay_group::OverlayTarget,
        panel_appearance_group::PanelAppearanceTarget,
        pending_upgrades_group::PendingUpgradesTarget, power_group::PowerTarget,
        quiet_schedule_group::QuietScheduleTarget, shell_layout_group::ShellLayoutTarget,
        surface_effects_group::SurfaceEffectsTarget, systemd_manager_group::SystemdTarget,
        wallpaper_group::WallpaperTarget, weather_group::WeatherTarget, window_group::WindowTarget,
    },
    widgets::phosphor::{Phosphor, Weight},
};

#[derive(Debug, Clone, PartialEq)]
pub enum HoverTarget {
    Compute(ComputeTarget),
    Gpu(GpuTarget),
    Disk(DiskTarget),
    Power(PowerTarget),
    PanelAppearance(PanelAppearanceTarget),
    SurfaceEffects(SurfaceEffectsTarget),
    NiriWindow(NiriWindowTarget),
    ShellLayout(ShellLayoutTarget),
    Overlay(OverlayTarget),
    Systemd(SystemdTarget),
    Journal(JournalTarget),
    PendingUpgrades(PendingUpgradesTarget),
    NetInterface(NetInterfaceTarget),
    Audio(ControlKind),
    Monitor(MonitorTarget),
    ColorTemperature(ColorTemperatureTarget),
    DarkMode(DarkModeTarget),
    IdleTimeout(TimerKind),
    Internet(InternetTarget),
    Weather(WeatherTarget),
    Wallpaper(WallpaperTarget),
    Media,
    Ai(AiTarget),
    FocusTime(FocusTimeTarget),
    Clock(ClockTarget),
    Notification(NotificationTarget),
    QuietSchedule(QuietScheduleTarget),
    Tray(String),
    Window(WindowTarget),
    Workspace(u64),
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
