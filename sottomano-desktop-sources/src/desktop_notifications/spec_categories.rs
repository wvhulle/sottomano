use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum NotificationCategory {
    Call,
    CallEnded,
    CallIncoming,
    CallUnanswered,
    Device,
    DeviceAdded,
    DeviceError,
    DeviceRemoved,
    Email,
    EmailArrived,
    EmailBounced,
    InstantMessage,
    InstantMessageError,
    InstantMessageReceived,
    Network,
    NetworkConnected,
    NetworkDisconnected,
    NetworkError,
    Presence,
    PresenceOffline,
    PresenceOnline,
    Transfer,
    TransferComplete,
    TransferError,
    Vendor(String),
    #[default]
    Unspecified,
}

struct SpecCategoryName {
    spec_name: &'static str,
    category: NotificationCategory,
}

const SPEC_CATEGORY_NAMES: [SpecCategoryName; 24] = [
    SpecCategoryName {
        spec_name: "call",
        category: NotificationCategory::Call,
    },
    SpecCategoryName {
        spec_name: "call.ended",
        category: NotificationCategory::CallEnded,
    },
    SpecCategoryName {
        spec_name: "call.incoming",
        category: NotificationCategory::CallIncoming,
    },
    SpecCategoryName {
        spec_name: "call.unanswered",
        category: NotificationCategory::CallUnanswered,
    },
    SpecCategoryName {
        spec_name: "device",
        category: NotificationCategory::Device,
    },
    SpecCategoryName {
        spec_name: "device.added",
        category: NotificationCategory::DeviceAdded,
    },
    SpecCategoryName {
        spec_name: "device.error",
        category: NotificationCategory::DeviceError,
    },
    SpecCategoryName {
        spec_name: "device.removed",
        category: NotificationCategory::DeviceRemoved,
    },
    SpecCategoryName {
        spec_name: "email",
        category: NotificationCategory::Email,
    },
    SpecCategoryName {
        spec_name: "email.arrived",
        category: NotificationCategory::EmailArrived,
    },
    SpecCategoryName {
        spec_name: "email.bounced",
        category: NotificationCategory::EmailBounced,
    },
    SpecCategoryName {
        spec_name: "im",
        category: NotificationCategory::InstantMessage,
    },
    SpecCategoryName {
        spec_name: "im.error",
        category: NotificationCategory::InstantMessageError,
    },
    SpecCategoryName {
        spec_name: "im.received",
        category: NotificationCategory::InstantMessageReceived,
    },
    SpecCategoryName {
        spec_name: "network",
        category: NotificationCategory::Network,
    },
    SpecCategoryName {
        spec_name: "network.connected",
        category: NotificationCategory::NetworkConnected,
    },
    SpecCategoryName {
        spec_name: "network.disconnected",
        category: NotificationCategory::NetworkDisconnected,
    },
    SpecCategoryName {
        spec_name: "network.error",
        category: NotificationCategory::NetworkError,
    },
    SpecCategoryName {
        spec_name: "presence",
        category: NotificationCategory::Presence,
    },
    SpecCategoryName {
        spec_name: "presence.offline",
        category: NotificationCategory::PresenceOffline,
    },
    SpecCategoryName {
        spec_name: "presence.online",
        category: NotificationCategory::PresenceOnline,
    },
    SpecCategoryName {
        spec_name: "transfer",
        category: NotificationCategory::Transfer,
    },
    SpecCategoryName {
        spec_name: "transfer.complete",
        category: NotificationCategory::TransferComplete,
    },
    SpecCategoryName {
        spec_name: "transfer.error",
        category: NotificationCategory::TransferError,
    },
];

impl From<&str> for NotificationCategory {
    fn from(category_name: &str) -> Self {
        SPEC_CATEGORY_NAMES
            .iter()
            .find(|spec_category_name| spec_category_name.spec_name == category_name)
            .map_or_else(
                || NotificationCategory::Vendor(category_name.to_owned()),
                |spec_category_name| spec_category_name.category.clone(),
            )
    }
}

impl fmt::Display for NotificationCategory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotificationCategory::Vendor(vendor_name) => formatter.write_str(vendor_name),
            NotificationCategory::Unspecified => formatter.write_str("unspecified"),
            spec_category => SPEC_CATEGORY_NAMES
                .iter()
                .find(|spec_category_name| spec_category_name.category == *spec_category)
                .map_or(Ok(()), |spec_category_name| {
                    formatter.write_str(spec_category_name.spec_name)
                }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_spec_category_name_round_trips() {
        SPEC_CATEGORY_NAMES.iter().for_each(|spec_category_name| {
            assert_eq!(
                NotificationCategory::from(spec_category_name.spec_name),
                spec_category_name.category
            );
            assert_eq!(
                spec_category_name.category.to_string(),
                spec_category_name.spec_name
            );
        });
    }

    #[test]
    fn an_unknown_name_is_a_vendor_category() {
        assert_eq!(
            NotificationCategory::from("x-gnome.music"),
            NotificationCategory::Vendor("x-gnome.music".to_owned())
        );
    }
}
