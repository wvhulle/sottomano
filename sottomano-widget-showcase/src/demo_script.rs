use std::{borrow::Cow, time::Duration};

use sottomano_sources::{
    login_session_presence::idle_session_actions::IdleAction, weather::open_meteo_pollen::Allergen,
};
use sottomano_widgets::{
    clock::Clock,
    group_menus::group_menu_state::{GroupMenuPin, GroupMenuTopic},
    hover_cards::HoverTarget,
    widget_groups::{
        appearance_presets_group::AppearancePresetsTarget,
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
        widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity},
        window_group::WindowTarget,
    },
};

use crate::{
    fixtures::{
        appearance_presets, audio, caldav, claude_usage, compute, gamepad, group_menu_members,
        idle_lock, login_session, media, memory_board, niri_session, notification_toasts,
        shell_settings, unchanging, voice_dictation, vpn, wallpaper, weather,
    },
    showcase_message::ShowcaseMessage,
    showcased_group::ShowcasedGroup,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupShown {
    OnPanel,
    InMenu(GroupMenuTopic),
    PinnedFrom {
        menu: GroupMenuTopic,
        group: WidgetGroupIdentity,
    },
}

pub struct Demo {
    pub name: &'static str,
    pub group: fn(Clock) -> ShowcasedGroup,
    pub live: fn(Clock) -> Vec<ShowcaseMessage>,
    pub shown: GroupShown,
    pub script: &'static [Step],
}

pub enum Step {
    Hover(HoverTarget),
    Press(HoverTarget),
    PressPin(GroupMenuPin),
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
        shown: GroupShown::InMenu(GroupMenuTopic::Hardware),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Hardware)),
            Step::HoverText("Compute"),
            Step::Hover(HoverTarget::Compute(ComputeTarget::CpuLoad)),
        ],
    },
    Demo {
        name: "compute_model",
        group: compute::group,
        live: compute::live,
        shown: GroupShown::InMenu(GroupMenuTopic::Hardware),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Hardware)),
            Step::HoverText("Compute"),
            Step::Hover(HoverTarget::Compute(ComputeTarget::CpuModel)),
        ],
    },
    Demo {
        name: "compute_clock",
        group: compute::group,
        live: compute::live,
        shown: GroupShown::InMenu(GroupMenuTopic::Hardware),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Hardware)),
            Step::HoverText("Compute"),
            Step::Hover(HoverTarget::Compute(ComputeTarget::CpuClock)),
        ],
    },
    Demo {
        name: "compute_cache_preference",
        group: compute::group,
        live: compute::live,
        shown: GroupShown::InMenu(GroupMenuTopic::Hardware),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Hardware)),
            Step::HoverText("Compute"),
            Step::Hover(HoverTarget::Compute(ComputeTarget::X3dCachePreference)),
        ],
    },
    Demo {
        name: "memory_board_memory",
        group: memory_board::group,
        live: memory_board::live,
        shown: GroupShown::InMenu(GroupMenuTopic::Hardware),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Hardware)),
            Step::HoverText("Memory"),
            Step::Hover(HoverTarget::MemoryBoard(MemoryBoardTarget::Memory)),
        ],
    },
    Demo {
        name: "memory_board_board",
        group: memory_board::group,
        live: memory_board::live,
        shown: GroupShown::InMenu(GroupMenuTopic::Hardware),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Hardware)),
            Step::HoverText("Memory"),
            Step::Hover(HoverTarget::MemoryBoard(MemoryBoardTarget::Board)),
        ],
    },
    Demo {
        name: "compute_temperature",
        group: compute::group,
        live: compute::live,
        shown: GroupShown::InMenu(GroupMenuTopic::Hardware),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Hardware)),
            Step::HoverText("Compute"),
            Step::Hover(HoverTarget::Compute(ComputeTarget::CpuTemperature)),
        ],
    },
    Demo {
        name: "vpn_tailscale",
        group: vpn::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Network),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Network)),
            Step::HoverText("VPN"),
            Step::Hover(HoverTarget::Vpn(VpnTarget::Tailscale)),
        ],
    },
    Demo {
        name: "vpn_wireguard",
        group: vpn::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Network),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Network)),
            Step::HoverText("VPN"),
            Step::Hover(HoverTarget::Vpn(VpnTarget::Wireguard(Cow::Borrowed(
                vpn::LAB_TUNNEL_NAME,
            )))),
            Step::Hover(HoverTarget::Vpn(VpnTarget::Wireguard(Cow::Borrowed(
                vpn::HOME_SERVER_TUNNEL_NAME,
            )))),
        ],
    },
    Demo {
        name: "weather_conditions",
        group: weather::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Agenda),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Agenda)),
            Step::HoverText("Weather"),
            Step::Hover(HoverTarget::Weather(WeatherTarget::Temperature)),
        ],
    },
    Demo {
        name: "weather_location",
        group: weather::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Agenda),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Agenda)),
            Step::HoverText("Weather"),
            Step::Hover(HoverTarget::Weather(WeatherTarget::Location)),
        ],
    },
    Demo {
        name: "weather_wind",
        group: weather::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Agenda),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Agenda)),
            Step::HoverText("Weather"),
            Step::Hover(HoverTarget::Weather(WeatherTarget::Wind)),
        ],
    },
    Demo {
        name: "weather_precipitation",
        group: weather::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Agenda),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Agenda)),
            Step::HoverText("Weather"),
            Step::Hover(HoverTarget::Weather(WeatherTarget::Rain)),
        ],
    },
    Demo {
        name: "weather_pollen",
        group: weather::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Agenda),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Agenda)),
            Step::HoverText("Weather"),
            Step::Hover(HoverTarget::Weather(WeatherTarget::Allergen(
                Allergen::Grass,
            ))),
        ],
    },
    Demo {
        name: "caldav_agenda",
        group: caldav::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Agenda),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Agenda)),
            Step::HoverText("Calendar"),
            Step::Hover(HoverTarget::Caldav(CaldavTarget::UpcomingEvents)),
            Step::Hover(HoverTarget::Caldav(CaldavTarget::OpenTasks)),
        ],
    },
    Demo {
        name: "ai_usage_windows",
        group: claude_usage::group,
        live: claude_usage::live,
        shown: GroupShown::InMenu(GroupMenuTopic::Services),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Services)),
            Step::HoverText("Claude usage"),
            Step::Hover(HoverTarget::ClaudeUsage(ClaudeUsageTarget::UsageWindows)),
        ],
    },
    Demo {
        name: "ai_usage_block",
        group: claude_usage::group,
        live: claude_usage::live,
        shown: GroupShown::InMenu(GroupMenuTopic::Services),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Services)),
            Step::HoverText("Claude usage"),
            Step::Hover(HoverTarget::ClaudeUsage(ClaudeUsageTarget::CcusageBlock)),
        ],
    },
    Demo {
        name: "ai_usage_periods",
        group: claude_usage::group,
        live: claude_usage::live,
        shown: GroupShown::InMenu(GroupMenuTopic::Services),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Services)),
            Step::HoverText("Claude usage"),
            Step::Hover(HoverTarget::ClaudeUsage(ClaudeUsageTarget::CcusagePeriods)),
        ],
    },
    Demo {
        name: "media_track",
        group: media::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Media),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Media)),
            Step::HoverText("Spotify"),
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
        shown: GroupShown::InMenu(GroupMenuTopic::Session),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Session)),
            Step::HoverText("Idle timeouts"),
            Step::Hover(HoverTarget::IdleTimeout(IdleAction::ScreenOff)),
        ],
    },
    Demo {
        name: "appearance_looks",
        group: appearance_presets::group,
        live: unchanging,
        shown: GroupShown::OnPanel,
        script: &[
            Step::Hover(HoverTarget::AppearancePresets(
                AppearancePresetsTarget::Menu,
            )),
            Step::PressText("Clear dark"),
            Step::RestFor(Duration::from_millis(1200)),
            Step::Hover(HoverTarget::AppearancePresets(
                AppearancePresetsTarget::Menu,
            )),
            Step::PressText("Bright office"),
            Step::RestFor(Duration::from_millis(1200)),
        ],
    },
    Demo {
        name: "idle_lock",
        group: idle_lock::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Session),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Session)),
            Step::HoverText("Idle timeouts"),
            Step::Hover(HoverTarget::IdleTimeout(IdleAction::Lock)),
        ],
    },
    Demo {
        name: "gamepad_controller",
        group: gamepad::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Media),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Media)),
            Step::HoverText("Gamepads"),
            Step::Hover(HoverTarget::Gamepad(GamepadTarget::Controller {
                slot: gamepad::DUALSENSE_SLOT,
            })),
        ],
    },
    Demo {
        name: "audio_volume",
        group: audio::group,
        live: audio::live,
        shown: GroupShown::OnPanel,
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
        shown: GroupShown::InMenu(GroupMenuTopic::Media),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Media)),
            Step::HoverText("Audio"),
            Step::Hover(HoverTarget::Audio(AudioTarget::Control(
                ControlKind::Source,
            ))),
        ],
    },
    Demo {
        name: "voice_dictation_model",
        group: voice_dictation::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Media),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Media)),
            Step::HoverText("Voice dictation"),
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
        name: "voice_dictation_pipeline",
        group: voice_dictation::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Media),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Media)),
            Step::HoverText("Voice dictation"),
            Step::Hover(HoverTarget::VoiceDictation(VoiceDictationTarget::Pipeline)),
        ],
    },
    Demo {
        name: "login_session_kernel",
        group: login_session::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Session),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Session)),
            Step::HoverText("Login session"),
            Step::Hover(HoverTarget::LoginSession(LoginSessionTarget::Kernel)),
            Step::Hover(HoverTarget::LoginSession(LoginSessionTarget::Compositor)),
        ],
    },
    Demo {
        name: "wallpaper_place",
        group: wallpaper::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Display),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Display)),
            Step::HoverText("Wallpaper"),
            Step::Hover(HoverTarget::Wallpaper(WallpaperTarget::Lookup)),
        ],
    },
    Demo {
        name: "wallpaper_article",
        group: wallpaper::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Display),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Display)),
            Step::HoverText("Wallpaper"),
            Step::Hover(HoverTarget::Wallpaper(WallpaperTarget::Lookup)),
            Step::Hover(HoverTarget::Wallpaper(WallpaperTarget::Article)),
        ],
    },
    Demo {
        name: "bottom_workspaces",
        group: niri_session::bottom_bar,
        live: unchanging,
        shown: GroupShown::OnPanel,
        script: &[Step::Hover(HoverTarget::Workspace(2))],
    },
    Demo {
        name: "bottom_window",
        group: niri_session::bottom_bar,
        live: unchanging,
        shown: GroupShown::OnPanel,
        script: &[Step::Hover(HoverTarget::Window(WindowTarget::Pick(2)))],
    },
    Demo {
        name: "bottom_process_inspector",
        group: niri_session::bottom_bar,
        live: unchanging,
        shown: GroupShown::OnPanel,
        script: &[Step::Hover(HoverTarget::Window(WindowTarget::Inspect(2)))],
    },
    Demo {
        name: "sidebar_workspaces",
        group: niri_session::workspace_sidebar,
        live: unchanging,
        shown: GroupShown::OnPanel,
        script: &[
            Step::Hover(HoverTarget::Workspace(1)),
            Step::Hover(HoverTarget::Workspace(4)),
            Step::Hover(HoverTarget::Workspace(6)),
            Step::Hover(HoverTarget::Workspace(9)),
        ],
    },
    Demo {
        name: "toast_arrival",
        group: notification_toasts::arrival,
        live: unchanging,
        shown: GroupShown::OnPanel,
        script: &[
            Step::HoverText(notification_toasts::ARRIVAL_SUMMARY),
            Step::RestFor(Duration::from_secs(5)),
        ],
    },
    Demo {
        name: "toast_progress",
        group: notification_toasts::progress,
        live: notification_toasts::downloading,
        shown: GroupShown::OnPanel,
        script: &[Step::RestFor(Duration::from_secs(10))],
    },
    Demo {
        name: "toast_actions",
        group: notification_toasts::actions,
        live: unchanging,
        shown: GroupShown::OnPanel,
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
        shown: GroupShown::OnPanel,
        script: &[Step::HoverText(notification_toasts::WINDOW_SUMMARY)],
    },
    Demo {
        name: "group_menu_media",
        group: group_menu_members::media_group_menu,
        live: audio::live,
        shown: GroupShown::InMenu(GroupMenuTopic::Media),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Media)),
            Step::HoverText("Spotify"),
            Step::HoverText("Audio"),
            Step::Hover(HoverTarget::Audio(AudioTarget::Control(
                ControlKind::Source,
            ))),
        ],
    },
    Demo {
        name: "group_menu_pin",
        group: group_menu_members::hardware_group_menu,
        live: group_menu_members::hardware_menu_live,
        shown: GroupShown::InMenu(GroupMenuTopic::Hardware),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Hardware)),
            Step::PressPin(GroupMenuPin::Group(WidgetGroupIdentity::sole(
                WidgetGroupFamily::MemoryBoard,
            ))),
            Step::RestFor(Duration::from_millis(1500)),
        ],
    },
    Demo {
        name: "group_menu_appearance",
        group: shell_settings::sharing_a_unit,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Appearance),
        script: &[Step::Hover(HoverTarget::GroupMenu(
            GroupMenuTopic::Appearance,
        ))],
    },
    Demo {
        name: "group_menu_behaviour",
        group: shell_settings::remaining,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Behaviour),
        script: &[
            Step::Hover(HoverTarget::GroupMenu(GroupMenuTopic::Behaviour)),
            Step::HoverText("Panels"),
        ],
    },
    Demo {
        name: "compute_cpu_pinned",
        group: compute::group,
        live: compute::live,
        shown: GroupShown::PinnedFrom {
            menu: GroupMenuTopic::Hardware,
            group: WidgetGroupIdentity::sole(WidgetGroupFamily::Compute),
        },
        script: &[
            Step::Hover(HoverTarget::Compute(ComputeTarget::CpuLoad)),
            Step::Press(HoverTarget::Compute(ComputeTarget::CpuLoad)),
            Step::Hover(HoverTarget::Compute(ComputeTarget::CpuTemperature)),
        ],
    },
    Demo {
        name: "memory_board_memory_pinned",
        group: memory_board::group,
        live: memory_board::live,
        shown: GroupShown::PinnedFrom {
            menu: GroupMenuTopic::Hardware,
            group: WidgetGroupIdentity::sole(WidgetGroupFamily::MemoryBoard),
        },
        script: &[
            Step::Hover(HoverTarget::MemoryBoard(MemoryBoardTarget::Memory)),
            Step::Press(HoverTarget::MemoryBoard(MemoryBoardTarget::Memory)),
            Step::Hover(HoverTarget::MemoryBoard(MemoryBoardTarget::Board)),
        ],
    },
    Demo {
        name: "vpn_wireguard_pinned",
        group: vpn::group,
        live: unchanging,
        shown: GroupShown::PinnedFrom {
            menu: GroupMenuTopic::Network,
            group: WidgetGroupIdentity::sole(WidgetGroupFamily::Vpn),
        },
        script: &[
            Step::Hover(HoverTarget::Vpn(VpnTarget::NetworkManagerVpn(
                Cow::Borrowed(vpn::COMPANY_VPN_NAME),
            ))),
            Step::Press(HoverTarget::Vpn(VpnTarget::NetworkManagerVpn(
                Cow::Borrowed(vpn::COMPANY_VPN_NAME),
            ))),
            Step::Hover(HoverTarget::Vpn(VpnTarget::Wireguard(Cow::Borrowed(
                vpn::LAB_TUNNEL_NAME,
            )))),
        ],
    },
    Demo {
        name: "wallpaper_place_pinned",
        group: wallpaper::group,
        live: unchanging,
        shown: GroupShown::PinnedFrom {
            menu: GroupMenuTopic::Display,
            group: WidgetGroupIdentity::sole(WidgetGroupFamily::Wallpaper),
        },
        script: &[
            Step::Hover(HoverTarget::Wallpaper(WallpaperTarget::Lookup)),
            Step::Press(HoverTarget::Wallpaper(WallpaperTarget::Lookup)),
            Step::Hover(HoverTarget::Wallpaper(WallpaperTarget::Article)),
        ],
    },
    Demo {
        name: "idle_screen_off_pinned",
        group: idle_lock::group,
        live: unchanging,
        shown: GroupShown::PinnedFrom {
            menu: GroupMenuTopic::Session,
            group: WidgetGroupIdentity::sole(WidgetGroupFamily::IdleTimeouts),
        },
        script: &[
            Step::Hover(HoverTarget::IdleTimeout(IdleAction::Lock)),
            Step::Press(HoverTarget::IdleTimeout(IdleAction::Lock)),
            Step::Hover(HoverTarget::IdleTimeout(IdleAction::ScreenOff)),
        ],
    },
    Demo {
        name: "ai_usage_windows_pinned",
        group: claude_usage::group,
        live: claude_usage::live,
        shown: GroupShown::PinnedFrom {
            menu: GroupMenuTopic::Services,
            group: WidgetGroupIdentity::sole(WidgetGroupFamily::ClaudeUsage),
        },
        script: &[
            Step::Hover(HoverTarget::ClaudeUsage(ClaudeUsageTarget::UsageWindows)),
            Step::Press(HoverTarget::ClaudeUsage(ClaudeUsageTarget::UsageWindows)),
            Step::Hover(HoverTarget::ClaudeUsage(ClaudeUsageTarget::CcusageBlock)),
        ],
    },
    Demo {
        name: "audio_microphone_pinned",
        group: audio::group,
        live: audio::live,
        shown: GroupShown::PinnedFrom {
            menu: GroupMenuTopic::Media,
            group: WidgetGroupIdentity::sole(WidgetGroupFamily::Audio),
        },
        script: &[
            Step::Hover(HoverTarget::Audio(AudioTarget::Control(ControlKind::Sink))),
            Step::Press(HoverTarget::Audio(AudioTarget::Control(ControlKind::Sink))),
            Step::Hover(HoverTarget::Audio(AudioTarget::Control(
                ControlKind::Source,
            ))),
        ],
    },
];
