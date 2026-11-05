pub mod anchored_popup;
pub mod core_grid;
pub mod hover_area;
pub mod hover_card_body;
pub mod hover_card_content;
pub mod hover_card_surfaces;
pub mod hover_card_transition;
pub mod laid_out_anchors;
pub use corredo::popup_connector;
use iced::window::Id;
use sottomano_desktop_sources::login_session_presence::idle_session_actions::IdleAction;
use sottomano_toml_config::screen_edges::Position;

pub use crate::hover_cards::{
    hover_area::{
        announcing_hoverable, hoverable, marked_by_hover_target, marked_by_menu_pin,
        press_toggled_hoverable,
    },
    hover_card_surfaces::HoverCardHost,
    laid_out_anchors::{CardOpening, WidgetAnchor},
};
use crate::{
    group_menus::{
        group_menu_settings::GroupMenuSettingsTarget,
        group_menu_state::{GroupMenuPin, GroupMenuTopic},
    },
    widget_groups::{
        app_launcher_group::AppLauncherTarget,
        appearance_presets_group::AppearancePresetsTarget,
        audio_group::AudioTarget,
        bluetooth_adapters_group::BluetoothTarget,
        caldav_group::CaldavTarget,
        claude_usage_group::ClaudeUsageTarget,
        clock_group::ClockTarget,
        color_temperature_group::ColorTemperatureTarget,
        compute_group::ComputeTarget,
        copilot_usage_group::CopilotUsageTarget,
        dark_mode_group::DarkModeTarget,
        ethernet_ports_group::EthernetPortTarget,
        gamepad_group::GamepadTarget,
        gpu_group::GpuTarget,
        journal_events_group::JournalTarget,
        llama_server_group::LlamaServerTarget,
        login_session_group::LoginSessionTarget,
        media_group::PlayerTarget,
        memory_board_group::MemoryBoardTarget,
        mobile_modems_group::MobileModemTarget,
        monitor_group::MonitorTarget,
        niri_window_group::NiriWindowTarget,
        notification_group::NotificationTarget,
        openrouter_credit_group::OpenRouterCreditTarget,
        overlay_settings_group::OverlaySettingsTarget,
        panel_settings_group::PanelSettingsTarget,
        pending_upgrades_group::PendingUpgradesTarget,
        physical_drive_group::DriveTarget,
        power_group::PowerTarget,
        quiet_schedule_group::QuietScheduleTarget,
        reachability_group::ReachabilityTarget,
        session_focus_group::SessionFocusTarget,
        session_power_group::SessionTarget,
        sidebar_settings_group::SidebarSettingsTarget,
        storage_space_group::StorageTarget,
        systemd_manager_group::SystemdTarget,
        tailnet_group::TailnetTarget,
        voice_dictation_group::VoiceDictationTarget,
        vpn_group::VpnTarget,
        wallpaper_group::WallpaperTarget,
        weather_group::WeatherTarget,
        widget_group_identity::{WidgetGroupIdentity, WidgetGroupTitle},
        wifi_adapters_group::WifiAdapterTarget,
        window_group::{WindowGroupMessage, WindowTarget},
        wireguard_server_group::WireguardServerTarget,
    },
    widgets::phosphor::{Phosphor, Weight},
};

#[derive(Debug, Clone, PartialEq)]
pub enum HoverTarget {
    Compute(ComputeTarget),
    MemoryBoard(MemoryBoardTarget),
    Gpu(GpuTarget),
    Storage(StorageTarget),
    Drive(DriveTarget),
    Power(PowerTarget),
    PanelSettings(PanelSettingsTarget),
    SidebarSettings(SidebarSettingsTarget),
    AppearancePresets(AppearancePresetsTarget),
    OverlaySettings(OverlaySettingsTarget),
    NiriWindow(NiriWindowTarget),
    Systemd(SystemdTarget),
    Journal(JournalTarget),
    PendingUpgrades(PendingUpgradesTarget),
    WifiAdapter(WifiAdapterTarget),
    MobileModem(MobileModemTarget),
    EthernetPort(EthernetPortTarget),
    Bluetooth(BluetoothTarget),
    Audio(AudioTarget),
    Monitor(MonitorTarget),
    ColorTemperature(ColorTemperatureTarget),
    DarkMode(DarkModeTarget),
    VoiceDictation(VoiceDictationTarget),
    IdleTimeout(IdleAction),
    PauseBlocker(PauseBlockerTarget),
    Reachability(ReachabilityTarget),
    Tailnet(TailnetTarget),
    WireguardServer(WireguardServerTarget),
    Vpn(VpnTarget),
    Weather(WeatherTarget),
    Wallpaper(WallpaperTarget),
    Caldav(CaldavTarget),
    Media(PlayerTarget),
    Gamepad(GamepadTarget),
    AppLauncher(AppLauncherTarget),
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
    WorkspaceFolding,
    GroupMenu(GroupMenuTopic),
    GroupFace(Box<WidgetGroupIdentity>),
    GroupMenuSettings(GroupMenuSettingsTarget),
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

impl HoverTarget {
    #[must_use]
    pub fn stable_item_key(&self) -> String {
        match self {
            HoverTarget::Media(player) => format!("Media({:?})", player.control),
            HoverTarget::Gamepad(target) => format!("Gamepad({})", target.stable_item_name()),
            durable_target => format!("{durable_target:?}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PauseBlockerTarget {
    pub pause: IdleAction,
    pub holder: String,
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum HoverOrigin {
    #[default]
    Reading,
    GroupHandle {
        group_title: Box<WidgetGroupTitle>,
        panel_pin: PanelPinOrigin,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum PanelPinOrigin {
    #[default]
    Unpinned,
    GroupPinnedFrom {
        menu: GroupMenuTopic,
        group: Box<WidgetGroupIdentity>,
    },
    ItemPinnedFrom {
        menu: GroupMenuTopic,
        group: Box<WidgetGroupIdentity>,
        item_key: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HoverAnnouncement {
    pub press_hints: PressHints,
    pub origin: HoverOrigin,
}

impl HoverAnnouncement {
    #[must_use]
    pub fn from_reading(press_hints: PressHints) -> Self {
        HoverAnnouncement {
            press_hints,
            origin: HoverOrigin::Reading,
        }
    }

    #[must_use]
    pub fn from_group_handle(group_title: WidgetGroupTitle, press_hints: PressHints) -> Self {
        HoverAnnouncement {
            press_hints,
            origin: HoverOrigin::GroupHandle {
                group_title: Box::new(group_title),
                panel_pin: PanelPinOrigin::Unpinned,
            },
        }
    }

    #[must_use]
    pub fn pinned_by(self, panel_pin: PanelPinOrigin) -> Self {
        match self.origin {
            HoverOrigin::GroupHandle { group_title, .. } => HoverAnnouncement {
                origin: HoverOrigin::GroupHandle {
                    group_title,
                    panel_pin,
                },
                ..self
            },
            HoverOrigin::Reading => self,
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.press_hints.is_empty() && self.origin == HoverOrigin::Reading
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MenuHandleIndicator {
    pub hovering: bool,
    pub open: bool,
}

#[derive(Debug, Clone)]
pub enum HoverCardMessage {
    AnchorLaidOut(WidgetAnchor),
    LaidOut {
        surface: Id,
        widget_anchor: WidgetAnchor,
    },
    EnterAnchor {
        target: HoverTarget,
        announcement: HoverAnnouncement,
    },
    Enter {
        surface: Id,
        position: Position,
        target: HoverTarget,
        announcement: HoverAnnouncement,
    },
    MenuHandlePointedAnchor {
        target: HoverTarget,
        pointed_at: bool,
        announcement: HoverAnnouncement,
    },
    MenuHandlePointed {
        surface: Id,
        position: Position,
        target: HoverTarget,
        pointed_at: bool,
        announcement: HoverAnnouncement,
    },
    MenuHandlePressedAnchor {
        target: HoverTarget,
        announcement: HoverAnnouncement,
    },
    MenuHandlePressed {
        surface: Id,
        position: Position,
        target: HoverTarget,
        announcement: HoverAnnouncement,
    },
    Exit {
        target: HoverTarget,
    },
    AnnouncementChanged {
        target: HoverTarget,
        announcement: HoverAnnouncement,
    },
    EnterHoverCard,
    ExitHoverCard,
    DismissHeldOpen,
    SwitchIntentElapsed,
    ShownWidgetDropped,
    ConfirmShownWidgetDropped,
}

#[derive(Debug, Clone)]
pub enum PopupMessage {
    HoverCard(HoverCardMessage),
    CopyText(String),
    OpenUrl(String),
    FocusWindow(u64),
    RefreshHoveredReading,
    WindowGroup(WindowGroupMessage),
    ToggleGroupMenuPin {
        menu: GroupMenuTopic,
        entry: GroupMenuPin,
    },
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::HoverTarget;
    use crate::widget_groups::{
        media_group::{MediaTarget, PlayerTarget},
        vpn_group::VpnTarget,
    };

    #[test]
    fn sibling_targets_have_their_own_item_keys() {
        let tunnel =
            |name: &'static str| HoverTarget::from(VpnTarget::Wireguard(Cow::Borrowed(name)));
        assert_ne!(
            tunnel("wg0").stable_item_key(),
            tunnel("wg1").stable_item_key()
        );
    }

    #[test]
    fn a_player_item_key_ignores_its_session_bus() {
        let on_bus = |bus: &str| HoverTarget::from(PlayerTarget::of(bus, MediaTarget::Track));
        assert_eq!(
            on_bus(":1.40").stable_item_key(),
            on_bus(":1.41").stable_item_key()
        );
    }

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
