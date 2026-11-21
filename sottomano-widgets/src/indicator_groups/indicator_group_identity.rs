use std::{
    borrow::Cow,
    collections::BTreeMap,
    fmt::{Display, Formatter, Result as FmtResult},
};

use strum::IntoStaticStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum IndicatorGroupFamily {
    AppearancePresets,
    ApptainerContainers,
    Audio,
    BluetoothAdapters,
    Caldav,
    CgroupMemoryLimits,
    CiRuns,
    ClaudeUsage,
    Clock,
    ColorTemperature,
    Compute,
    CopilotUsage,
    DarkMode,
    DockerEngines,
    EthernetPorts,
    FlatpakSandboxes,
    Gamepad,
    #[strum(serialize = "global_menu_switch")]
    WindowMenuBarSwitch,
    Gpu,
    GroupMenuSettings,
    IdleTimeouts,
    IncusInstances,
    MicrovmGuests,
    LlamaServer,
    LoginSession,
    LxcContainers,
    Media,
    MemoryBoard,
    MobileModems,
    Monitor,
    SteamDownloads,
    SteamLibrary,
    SteamRunningGame,
    CompositorStyling,
    Notification,
    NspawnContainers,
    OpenRouterCredit,
    OverlaySettings,
    PanelSettings,
    PendingUpgrades,
    PhysicalDrive,
    PodmanEngines,
    Power,
    QuietSchedule,
    Reachability,
    SessionFocus,
    SidebarSettings,
    StorageSpace,
    SystemJournal,
    SystemdManager,
    Tailnet,
    Tray,
    UserJournal,
    VirtualBoxMachines,
    VirtualMachines,
    VmwareMachines,
    VoiceDictation,
    NetworkManagerVpns,
    Wallpaper,
    WaydroidContainer,
    Weather,
    WifiAdapters,
    WireguardTunnels,
}

impl IndicatorGroupFamily {
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            IndicatorGroupFamily::AppearancePresets => "Looks",
            IndicatorGroupFamily::Audio => "Audio",
            IndicatorGroupFamily::BluetoothAdapters => "Bluetooth",
            IndicatorGroupFamily::Caldav => "Calendar",
            IndicatorGroupFamily::CgroupMemoryLimits => "Memory limits",
            IndicatorGroupFamily::CiRuns => "CI runs",
            IndicatorGroupFamily::ClaudeUsage => "Claude usage",
            IndicatorGroupFamily::Clock => "Clock",
            IndicatorGroupFamily::ColorTemperature => "Color temperature",
            IndicatorGroupFamily::Compute => "Compute",
            IndicatorGroupFamily::CopilotUsage => "Copilot usage",
            IndicatorGroupFamily::DarkMode => "Dark mode",
            IndicatorGroupFamily::EthernetPorts => "Ethernet",
            IndicatorGroupFamily::Gamepad => "Gamepads",
            IndicatorGroupFamily::Gpu => "GPU",
            IndicatorGroupFamily::GroupMenuSettings => "Menus",
            IndicatorGroupFamily::IdleTimeouts => "Idle timeouts",
            IndicatorGroupFamily::LlamaServer => "Llama server",
            IndicatorGroupFamily::DockerEngines => "Docker containers",
            IndicatorGroupFamily::MicrovmGuests => "microvm.nix guests",
            IndicatorGroupFamily::IncusInstances => "Incus instances",
            IndicatorGroupFamily::LxcContainers => "LXC containers",
            IndicatorGroupFamily::FlatpakSandboxes => "Flatpak apps",
            IndicatorGroupFamily::WaydroidContainer => "Waydroid",
            IndicatorGroupFamily::UserJournal => "User journal",
            IndicatorGroupFamily::VirtualBoxMachines => "VirtualBox machines",
            IndicatorGroupFamily::VmwareMachines => "VMware machines",
            IndicatorGroupFamily::ApptainerContainers => "Apptainer containers",
            IndicatorGroupFamily::PodmanEngines => "Podman containers",
            IndicatorGroupFamily::VirtualMachines => "QEMU machines",
            IndicatorGroupFamily::NspawnContainers => "nspawn containers",
            IndicatorGroupFamily::LoginSession => "Login session",
            IndicatorGroupFamily::Media => "Media",
            IndicatorGroupFamily::MemoryBoard => "Memory",
            IndicatorGroupFamily::MobileModems => "Mobile modem",
            IndicatorGroupFamily::SteamDownloads => "Steam downloads",
            IndicatorGroupFamily::SteamLibrary => "Steam library",
            IndicatorGroupFamily::SteamRunningGame => "Steam game",
            IndicatorGroupFamily::Monitor => "Monitor",
            IndicatorGroupFamily::CompositorStyling => "Window styling",
            IndicatorGroupFamily::Notification => "Notifications",
            IndicatorGroupFamily::OpenRouterCredit => "OpenRouter credit",
            IndicatorGroupFamily::OverlaySettings => "Overlays",
            IndicatorGroupFamily::PanelSettings => "Panels",
            IndicatorGroupFamily::PendingUpgrades => "Pending upgrades",
            IndicatorGroupFamily::PhysicalDrive => "Drive",
            IndicatorGroupFamily::Power => "Power",
            IndicatorGroupFamily::QuietSchedule => "Quiet schedule",
            IndicatorGroupFamily::Reachability => "Reachability",
            IndicatorGroupFamily::SessionFocus => "Focus session",
            IndicatorGroupFamily::SidebarSettings => "Sidebar",
            IndicatorGroupFamily::StorageSpace => "Filesystem",
            IndicatorGroupFamily::SystemJournal => "System journal",
            IndicatorGroupFamily::SystemdManager => "Systemd",
            IndicatorGroupFamily::Tailnet => "Tailscale",
            IndicatorGroupFamily::Tray => "Tray",
            IndicatorGroupFamily::WindowMenuBarSwitch => "Menu bar",
            IndicatorGroupFamily::VoiceDictation => "Voice dictation",
            IndicatorGroupFamily::NetworkManagerVpns => "VPN",
            IndicatorGroupFamily::Wallpaper => "Wallpaper",
            IndicatorGroupFamily::Weather => "Weather",
            IndicatorGroupFamily::WifiAdapters => "Wi-Fi",
            IndicatorGroupFamily::WireguardTunnels => "WireGuard",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IndicatorGroupInstance {
    Sole,
    Durable(Cow<'static, str>),
    Session {
        durable: Cow<'static, str>,
        session: Cow<'static, str>,
    },
}

impl IndicatorGroupInstance {
    #[must_use]
    pub fn stored_name(&self) -> &str {
        match self {
            IndicatorGroupInstance::Sole => "",
            IndicatorGroupInstance::Durable(durable)
            | IndicatorGroupInstance::Session { durable, .. } => durable,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IndicatorGroupIdentity {
    pub family: IndicatorGroupFamily,
    pub instance: IndicatorGroupInstance,
}

impl IndicatorGroupIdentity {
    #[must_use]
    pub const fn sole(family: IndicatorGroupFamily) -> Self {
        Self {
            family,
            instance: IndicatorGroupInstance::Sole,
        }
    }

    #[must_use]
    pub fn durable(family: IndicatorGroupFamily, durable: impl Into<Cow<'static, str>>) -> Self {
        Self {
            family,
            instance: IndicatorGroupInstance::Durable(durable.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndicatorGroupTitle {
    Named(Cow<'static, str>),
    Numbered {
        base_title: Cow<'static, str>,
        ordinal: u8,
    },
}

impl IndicatorGroupTitle {
    #[must_use]
    pub const fn fixed(title: &'static str) -> Self {
        IndicatorGroupTitle::Named(Cow::Borrowed(title))
    }

    #[must_use]
    pub fn numbered_from_second(base_title: impl Into<Cow<'static, str>>, position: usize) -> Self {
        let base_title = base_title.into();
        match u8::try_from(position + 1) {
            Ok(ordinal) if ordinal > 1 => IndicatorGroupTitle::Numbered {
                base_title,
                ordinal,
            },
            Ok(_) | Err(_) => IndicatorGroupTitle::Named(base_title),
        }
    }

    pub fn numbered_namesakes(
        names: impl IntoIterator<Item: Into<Cow<'static, str>>>,
    ) -> Vec<Self> {
        let mut namesake_counts: BTreeMap<Cow<'static, str>, usize> = BTreeMap::new();
        names
            .into_iter()
            .map(|name| {
                let name = name.into();
                let namesake_count = namesake_counts.entry(name.clone()).or_default();
                let title = Self::numbered_from_second(name, *namesake_count);
                *namesake_count += 1;
                title
            })
            .collect()
    }
}

impl Display for IndicatorGroupTitle {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match self {
            IndicatorGroupTitle::Named(title) => formatter.write_str(title),
            IndicatorGroupTitle::Numbered {
                base_title,
                ordinal,
            } => write!(formatter, "{base_title} {ordinal}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{IndicatorGroupFamily, IndicatorGroupIdentity, IndicatorGroupInstance};

    #[test]
    fn a_family_stores_a_name_independent_of_its_title() {
        assert_eq!(
            <&'static str>::from(IndicatorGroupFamily::SessionFocus),
            "session_focus"
        );
        assert_eq!(IndicatorGroupFamily::SessionFocus.title(), "Focus session");
    }

    #[test]
    fn namesake_session_instances_differ_but_store_the_same_name() {
        let player_on_bus = |session: &'static str| IndicatorGroupIdentity {
            family: IndicatorGroupFamily::Media,
            instance: IndicatorGroupInstance::Session {
                durable: "Firefox".into(),
                session: session.into(),
            },
        };
        assert_ne!(player_on_bus(":1.40"), player_on_bus(":1.41"));
        assert_eq!(
            player_on_bus(":1.40").instance.stored_name(),
            player_on_bus(":1.41").instance.stored_name()
        );
    }
}
