#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModemFailure {
    SimMissing,
    SimError,
    UnknownCapabilities,
    EsimWithoutProfiles,
    Unspecified,
}

impl From<u32> for ModemFailure {
    fn from(reason_code: u32) -> Self {
        match reason_code {
            2 => Self::SimMissing,
            3 => Self::SimError,
            4 => Self::UnknownCapabilities,
            5 => Self::EsimWithoutProfiles,
            _ => Self::Unspecified,
        }
    }
}

impl ModemFailure {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::SimMissing => "sim-missing",
            Self::SimError => "sim-error",
            Self::UnknownCapabilities => "unknown-capabilities",
            Self::EsimWithoutProfiles => "esim-without-profiles",
            Self::Unspecified => "unspecified",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModemState {
    Unreported,
    Failed { reason: ModemFailure },
    Initializing,
    Locked,
    Disabled,
    Disabling,
    Enabling,
    Enabled,
    Searching,
    Registered,
    Disconnecting,
    Connecting,
    Connected,
}

impl TryFrom<i32> for ModemState {
    type Error = i32;

    fn try_from(state_code: i32) -> Result<Self, Self::Error> {
        match state_code {
            -1 => Ok(Self::Failed {
                reason: ModemFailure::Unspecified,
            }),
            1 => Ok(Self::Initializing),
            2 => Ok(Self::Locked),
            3 => Ok(Self::Disabled),
            4 => Ok(Self::Disabling),
            5 => Ok(Self::Enabling),
            6 => Ok(Self::Enabled),
            7 => Ok(Self::Searching),
            8 => Ok(Self::Registered),
            9 => Ok(Self::Disconnecting),
            10 => Ok(Self::Connecting),
            11 => Ok(Self::Connected),
            unknown => Err(unknown),
        }
    }
}

impl ModemState {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Unreported => "unreported",
            Self::Failed { .. } => "failed",
            Self::Initializing => "initializing",
            Self::Locked => "locked",
            Self::Disabled => "disabled",
            Self::Disabling => "disabling",
            Self::Enabling => "enabling",
            Self::Enabled => "enabled",
            Self::Searching => "searching",
            Self::Registered => "registered",
            Self::Disconnecting => "disconnecting",
            Self::Connecting => "connecting",
            Self::Connected => "connected",
        }
    }

    pub(crate) fn with_failure_reason(self, reason: ModemFailure) -> Self {
        match self {
            Self::Failed { .. } => Self::Failed { reason },
            other => other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkRegistration {
    Idle,
    Home,
    Searching,
    Denied,
    Roaming,
    HomeSmsOnly,
    RoamingSmsOnly,
    EmergencyOnly,
    HomeCsfbNotPreferred,
    RoamingCsfbNotPreferred,
    AttachedRlos,
    Unreported,
}

impl From<u32> for NetworkRegistration {
    fn from(state_code: u32) -> Self {
        match state_code {
            0 => Self::Idle,
            1 => Self::Home,
            2 => Self::Searching,
            3 => Self::Denied,
            5 => Self::Roaming,
            6 => Self::HomeSmsOnly,
            7 => Self::RoamingSmsOnly,
            8 => Self::EmergencyOnly,
            9 => Self::HomeCsfbNotPreferred,
            10 => Self::RoamingCsfbNotPreferred,
            11 => Self::AttachedRlos,
            _ => Self::Unreported,
        }
    }
}

impl NetworkRegistration {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Home => "home",
            Self::Searching => "searching",
            Self::Denied => "denied",
            Self::Roaming => "roaming",
            Self::HomeSmsOnly => "home-sms-only",
            Self::RoamingSmsOnly => "roaming-sms-only",
            Self::EmergencyOnly => "emergency-only",
            Self::HomeCsfbNotPreferred => "home-csfb-not-preferred",
            Self::RoamingCsfbNotPreferred => "roaming-csfb-not-preferred",
            Self::AttachedRlos => "attached-rlos",
            Self::Unreported => "unreported",
        }
    }
}
