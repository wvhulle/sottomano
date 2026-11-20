use std::path::PathBuf;

use sottomano_desktop_sources::desktop_notifications::{
    notification_data::NotificationData, spec_categories::NotificationCategory,
    spec_hints::SoundRequest,
};
use sottomano_toml_config::severity::Severity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SoundChoice {
    Silent,
    RequestedFile(PathBuf),
    RequestedThemeSound(String),
    PolicyThemeSounds(&'static [&'static str]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastSlot {
    Vacant,
    Occupied { shown_severity: Severity },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CategorySounds {
    Chain(&'static [&'static str]),
    Unmapped,
}

fn category_sounds(category: &NotificationCategory) -> CategorySounds {
    match category {
        NotificationCategory::DeviceAdded => CategorySounds::Chain(&["device-added"]),
        NotificationCategory::DeviceRemoved => CategorySounds::Chain(&["device-removed"]),
        NotificationCategory::DeviceError => {
            CategorySounds::Chain(&["device-error", "dialog-error"])
        }
        NotificationCategory::CallIncoming => CategorySounds::Chain(&["phone-incoming-call"]),
        NotificationCategory::CallUnanswered => CategorySounds::Chain(&["phone-outgoing-busy"]),
        NotificationCategory::CallEnded => {
            CategorySounds::Chain(&["phone-hangup", "service-logout"])
        }
        NotificationCategory::EmailArrived | NotificationCategory::Email => {
            CategorySounds::Chain(&["message-new-email", "message-new-instant"])
        }
        NotificationCategory::EmailBounced
        | NotificationCategory::InstantMessageError
        | NotificationCategory::TransferError => CategorySounds::Chain(&["dialog-error"]),
        NotificationCategory::InstantMessageReceived | NotificationCategory::InstantMessage => {
            CategorySounds::Chain(&["message-new-instant"])
        }
        NotificationCategory::NetworkConnected => {
            CategorySounds::Chain(&["network-connectivity-established"])
        }
        NotificationCategory::NetworkDisconnected => {
            CategorySounds::Chain(&["network-connectivity-lost"])
        }
        NotificationCategory::NetworkError => {
            CategorySounds::Chain(&["network-connectivity-error", "dialog-error"])
        }
        NotificationCategory::PresenceOnline => CategorySounds::Chain(&["service-login"]),
        NotificationCategory::PresenceOffline => CategorySounds::Chain(&["service-logout"]),
        NotificationCategory::TransferComplete => CategorySounds::Chain(&["complete"]),
        NotificationCategory::Call
        | NotificationCategory::Device
        | NotificationCategory::Network
        | NotificationCategory::Presence
        | NotificationCategory::Transfer
        | NotificationCategory::Vendor(_)
        | NotificationCategory::Unspecified => CategorySounds::Unmapped,
    }
}

fn urgency_sounds(data: &NotificationData) -> &'static [&'static str] {
    match data.severity {
        Severity::Critical => &["dialog-warning", "window-attention"],
        Severity::Warning => &["dialog-warning"],
        Severity::Normal if !data.actions.is_empty() => &["window-question"],
        Severity::Normal => &["message"],
        Severity::Low => &["dialog-information"],
    }
}

#[must_use]
pub fn sound_choice(data: &NotificationData, toast_slot: ToastSlot) -> SoundChoice {
    if let ToastSlot::Occupied { shown_severity } = toast_slot
        && data.severity <= shown_severity
    {
        return SoundChoice::Silent;
    }
    match &data.sound_request {
        SoundRequest::Suppressed => return SoundChoice::Silent,
        SoundRequest::File(sound_file) => return SoundChoice::RequestedFile(sound_file.clone()),
        SoundRequest::ThemeName(sound_name) => {
            return SoundChoice::RequestedThemeSound(sound_name.clone());
        }
        SoundRequest::Unspecified => {}
    }
    match category_sounds(&data.category) {
        CategorySounds::Chain(sound_names) => SoundChoice::PolicyThemeSounds(sound_names),
        CategorySounds::Unmapped => SoundChoice::PolicyThemeSounds(urgency_sounds(data)),
    }
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::desktop_notifications::notification_data::{
        ActionTint, NotifAction,
    };

    use super::*;

    fn notification(category: &str, severity: Severity) -> NotificationData {
        NotificationData {
            category: NotificationCategory::from(category),
            severity,
            ..NotificationData::default()
        }
    }

    struct ExpectedSound {
        category: &'static str,
        sound_name: &'static str,
    }

    fn first_sound(data: &NotificationData) -> &'static str {
        match sound_choice(data, ToastSlot::Vacant) {
            SoundChoice::PolicyThemeSounds(sound_names) => sound_names[0],
            other_choice => panic!("expected a policy sound, got {other_choice:?}"),
        }
    }

    #[test]
    fn every_mapped_category_has_its_own_first_sound() {
        [
            ExpectedSound {
                category: "device.added",
                sound_name: "device-added",
            },
            ExpectedSound {
                category: "device.removed",
                sound_name: "device-removed",
            },
            ExpectedSound {
                category: "device.error",
                sound_name: "device-error",
            },
            ExpectedSound {
                category: "call.incoming",
                sound_name: "phone-incoming-call",
            },
            ExpectedSound {
                category: "call.unanswered",
                sound_name: "phone-outgoing-busy",
            },
            ExpectedSound {
                category: "call.ended",
                sound_name: "phone-hangup",
            },
            ExpectedSound {
                category: "email.arrived",
                sound_name: "message-new-email",
            },
            ExpectedSound {
                category: "email.bounced",
                sound_name: "dialog-error",
            },
            ExpectedSound {
                category: "im.received",
                sound_name: "message-new-instant",
            },
            ExpectedSound {
                category: "im.error",
                sound_name: "dialog-error",
            },
            ExpectedSound {
                category: "network.connected",
                sound_name: "network-connectivity-established",
            },
            ExpectedSound {
                category: "network.disconnected",
                sound_name: "network-connectivity-lost",
            },
            ExpectedSound {
                category: "network.error",
                sound_name: "network-connectivity-error",
            },
            ExpectedSound {
                category: "presence.online",
                sound_name: "service-login",
            },
            ExpectedSound {
                category: "presence.offline",
                sound_name: "service-logout",
            },
            ExpectedSound {
                category: "transfer.complete",
                sound_name: "complete",
            },
            ExpectedSound {
                category: "transfer.error",
                sound_name: "dialog-error",
            },
        ]
        .iter()
        .for_each(|expected_sound| {
            assert_eq!(
                first_sound(&notification(expected_sound.category, Severity::Normal)),
                expected_sound.sound_name,
                "{}",
                expected_sound.category
            );
        });
    }

    #[test]
    fn a_category_outranks_actions() {
        let mut device_plug = notification("device.added", Severity::Normal);
        device_plug.actions = vec![NotifAction {
            key: "default".to_owned(),
            label: "Open".to_owned(),
            tint: ActionTint::Proceed,
        }];
        assert_eq!(first_sound(&device_plug), "device-added");
    }

    #[test]
    fn unmapped_notifications_follow_their_urgency() {
        assert_eq!(
            first_sound(&notification("x-app", Severity::Low)),
            "dialog-information"
        );
        assert_eq!(
            first_sound(&notification("x-app", Severity::Normal)),
            "message"
        );
        assert_eq!(
            first_sound(&notification("x-app", Severity::Critical)),
            "dialog-warning"
        );
    }

    #[test]
    fn spec_sound_hints_outrank_policy() {
        let mut requested = notification("device.added", Severity::Normal);
        requested.sound_request = SoundRequest::ThemeName("bell".to_owned());
        assert_eq!(
            sound_choice(&requested, ToastSlot::Vacant),
            SoundChoice::RequestedThemeSound("bell".to_owned())
        );
        requested.sound_request = SoundRequest::Suppressed;
        assert_eq!(
            sound_choice(&requested, ToastSlot::Vacant),
            SoundChoice::Silent
        );
    }

    #[test]
    fn refreshing_a_shown_toast_at_the_same_severity_is_silent() {
        let ram_refresh = notification("x-app", Severity::Critical);
        assert_eq!(
            sound_choice(
                &ram_refresh,
                ToastSlot::Occupied {
                    shown_severity: Severity::Critical
                }
            ),
            SoundChoice::Silent
        );
    }

    #[test]
    fn calming_a_shown_toast_is_silent() {
        let calmer_refresh = notification("x-app", Severity::Warning);
        assert_eq!(
            sound_choice(
                &calmer_refresh,
                ToastSlot::Occupied {
                    shown_severity: Severity::Critical
                }
            ),
            SoundChoice::Silent
        );
    }

    #[test]
    fn requested_sounds_stay_silent_on_an_occupied_slot() {
        let mut requested_refresh = notification("x-app", Severity::Normal);
        requested_refresh.sound_request = SoundRequest::ThemeName("bell".to_owned());
        assert_eq!(
            sound_choice(
                &requested_refresh,
                ToastSlot::Occupied {
                    shown_severity: Severity::Normal
                }
            ),
            SoundChoice::Silent
        );
    }

    #[test]
    fn escalating_a_shown_toast_chimes() {
        let escalated_refresh = notification("x-app", Severity::Critical);
        assert_eq!(
            sound_choice(
                &escalated_refresh,
                ToastSlot::Occupied {
                    shown_severity: Severity::Warning
                }
            ),
            SoundChoice::PolicyThemeSounds(&["dialog-warning", "window-attention"])
        );
    }
}
