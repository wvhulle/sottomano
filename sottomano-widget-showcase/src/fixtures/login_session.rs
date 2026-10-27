use std::num::NonZeroU32;

use sottomano_sources::{
    SnapshotDelivery,
    host_machine_identity::hostname1_os_identity::{Chassis, OperatingSystemIdentity},
    login_session_presence::logind_session_facts::SessionIdentity,
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::login_session_group::{LoginSessionGroup, LoginSessionMessage},
};

use crate::showcased_group::ShowcasedGroup;

pub fn group(clock: Clock) -> ShowcasedGroup {
    ShowcasedGroup::LoginSession(Box::new(session_readings().into_iter().fold(
        LoginSessionGroup::default(),
        |mut group, reading| {
            let _ = group.update(reading, clock);
            group
        },
    )))
}

fn session_readings() -> [LoginSessionMessage; 2] {
    [
        LoginSessionMessage::SystemFetched(SnapshotDelivery::live_push(Ok(
            OperatingSystemIdentity {
                hostname: "workstation".to_owned(),
                kernel: "6.18.2".to_owned(),
                operating_system: "NixOS 26.05 (Yarara)".to_owned(),
                distribution_ids: vec!["nixos".to_owned()],
                chassis: Some(Chassis::Desktop),
            },
        ))),
        LoginSessionMessage::IdentityFetched(SnapshotDelivery::live_push(Ok(SessionIdentity {
            user: "demo".to_owned(),
            uid: 1000,
            seat: "seat0".to_owned(),
            virtual_terminal: NonZeroU32::new(2),
            login_manager: "greetd".to_owned(),
            desktop: Some("niri".to_owned()),
            session_type: "wayland".to_owned(),
        }))),
    ]
}
