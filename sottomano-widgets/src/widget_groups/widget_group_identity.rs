use std::{
    borrow::Cow,
    fmt::{Display, Formatter, Result as FmtResult},
};

use strum::IntoStaticStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum WidgetGroupFamily {
    AppLauncher,
    Audio,
    BluetoothAdapters,
    Caldav,
    ClaudeUsage,
    Clock,
    ColorTemperature,
    Compute,
    CopilotUsage,
    DarkMode,
    EthernetPorts,
    Gamepad,
    Gpu,
    GroupMenuSettings,
    IdleTimeouts,
    JournalEvents,
    ForgejoTasks,
    LlamaServer,
    LoginSession,
    Media,
    MemoryBoard,
    MobileModems,
    Monitor,
    NiriWindow,
    Notification,
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
    SystemdManager,
    Tailnet,
    Tray,
    VirtualMachines,
    VoiceDictation,
    Vpn,
    Wallpaper,
    Weather,
    WifiAdapters,
    WireguardServer,
    WoodpeckerTasks,
}

impl WidgetGroupFamily {
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            WidgetGroupFamily::AppLauncher => "Applications",
            WidgetGroupFamily::Audio => "Audio",
            WidgetGroupFamily::BluetoothAdapters => "Bluetooth",
            WidgetGroupFamily::Caldav => "Calendar",
            WidgetGroupFamily::ClaudeUsage => "Claude usage",
            WidgetGroupFamily::Clock => "Clock",
            WidgetGroupFamily::ColorTemperature => "Color temperature",
            WidgetGroupFamily::Compute => "Compute",
            WidgetGroupFamily::CopilotUsage => "Copilot usage",
            WidgetGroupFamily::DarkMode => "Dark mode",
            WidgetGroupFamily::EthernetPorts => "Ethernet",
            WidgetGroupFamily::Gamepad => "Gamepads",
            WidgetGroupFamily::Gpu => "GPU",
            WidgetGroupFamily::GroupMenuSettings => "Menus",
            WidgetGroupFamily::IdleTimeouts => "Idle timeouts",
            WidgetGroupFamily::JournalEvents => "Journal",
            WidgetGroupFamily::ForgejoTasks => "Forgejo",
            WidgetGroupFamily::LlamaServer => "Llama server",
            WidgetGroupFamily::PodmanEngines => "Podman",
            WidgetGroupFamily::VirtualMachines => "VMs",
            WidgetGroupFamily::WoodpeckerTasks => "Woodpecker",
            WidgetGroupFamily::LoginSession => "Login session",
            WidgetGroupFamily::Media => "Media",
            WidgetGroupFamily::MemoryBoard => "Memory",
            WidgetGroupFamily::MobileModems => "Mobile modem",
            WidgetGroupFamily::Monitor => "Monitor",
            WidgetGroupFamily::NiriWindow => "Niri",
            WidgetGroupFamily::Notification => "Notifications",
            WidgetGroupFamily::OpenRouterCredit => "OpenRouter credit",
            WidgetGroupFamily::OverlaySettings => "Overlays",
            WidgetGroupFamily::PanelSettings => "Panels",
            WidgetGroupFamily::PendingUpgrades => "Pending upgrades",
            WidgetGroupFamily::PhysicalDrive => "Drive",
            WidgetGroupFamily::Power => "Power",
            WidgetGroupFamily::QuietSchedule => "Quiet schedule",
            WidgetGroupFamily::Reachability => "Reachability",
            WidgetGroupFamily::SessionFocus => "Focus session",
            WidgetGroupFamily::SidebarSettings => "Sidebar",
            WidgetGroupFamily::StorageSpace => "Filesystem",
            WidgetGroupFamily::SystemdManager => "Systemd",
            WidgetGroupFamily::Tailnet => "Tailscale",
            WidgetGroupFamily::Tray => "Tray",
            WidgetGroupFamily::VoiceDictation => "Voice dictation",
            WidgetGroupFamily::Vpn => "VPN",
            WidgetGroupFamily::Wallpaper => "Wallpaper",
            WidgetGroupFamily::Weather => "Weather",
            WidgetGroupFamily::WifiAdapters => "Wi-Fi",
            WidgetGroupFamily::WireguardServer => "WireGuard server",
        }
    }

    #[must_use]
    pub fn stored_name(self) -> &'static str {
        self.into()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WidgetGroupInstance {
    Sole,
    Durable(Cow<'static, str>),
    Session {
        durable: Cow<'static, str>,
        session: Cow<'static, str>,
    },
}

impl WidgetGroupInstance {
    #[must_use]
    pub fn stored_name(&self) -> &str {
        match self {
            WidgetGroupInstance::Sole => "",
            WidgetGroupInstance::Durable(durable)
            | WidgetGroupInstance::Session { durable, .. } => durable,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WidgetGroupIdentity {
    pub family: WidgetGroupFamily,
    pub instance: WidgetGroupInstance,
}

impl WidgetGroupIdentity {
    #[must_use]
    pub const fn sole(family: WidgetGroupFamily) -> Self {
        Self {
            family,
            instance: WidgetGroupInstance::Sole,
        }
    }

    #[must_use]
    pub fn durable(family: WidgetGroupFamily, durable: impl Into<Cow<'static, str>>) -> Self {
        Self {
            family,
            instance: WidgetGroupInstance::Durable(durable.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WidgetGroupTitle {
    Named(Cow<'static, str>),
    Numbered {
        base_title: Cow<'static, str>,
        ordinal: u8,
    },
}

impl WidgetGroupTitle {
    #[must_use]
    pub const fn fixed(title: &'static str) -> Self {
        WidgetGroupTitle::Named(Cow::Borrowed(title))
    }

    #[must_use]
    pub fn numbered_from_second(base_title: impl Into<Cow<'static, str>>, position: usize) -> Self {
        let base_title = base_title.into();
        match u8::try_from(position + 1) {
            Ok(ordinal) if ordinal > 1 => WidgetGroupTitle::Numbered {
                base_title,
                ordinal,
            },
            Ok(_) | Err(_) => WidgetGroupTitle::Named(base_title),
        }
    }
}

impl Display for WidgetGroupTitle {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match self {
            WidgetGroupTitle::Named(title) => formatter.write_str(title),
            WidgetGroupTitle::Numbered {
                base_title,
                ordinal,
            } => write!(formatter, "{base_title} {ordinal}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{WidgetGroupFamily, WidgetGroupIdentity, WidgetGroupInstance};

    #[test]
    fn a_family_stores_a_name_independent_of_its_title() {
        assert_eq!(
            WidgetGroupFamily::SessionFocus.stored_name(),
            "session_focus"
        );
        assert_eq!(WidgetGroupFamily::SessionFocus.title(), "Focus session");
    }

    #[test]
    fn namesake_session_instances_differ_but_store_the_same_name() {
        let player_on_bus = |session: &'static str| WidgetGroupIdentity {
            family: WidgetGroupFamily::Media,
            instance: WidgetGroupInstance::Session {
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
