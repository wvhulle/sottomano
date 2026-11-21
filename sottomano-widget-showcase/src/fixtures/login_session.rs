use std::num::NonZeroU32;

use sottomano_desktop_sources::{
    SnapshotDelivery,
    host_machine_identity::hostname1_os_identity::{Chassis, OperatingSystemIdentity},
    login_session_presence::logind_session_facts::{SessionIdentity, VirtualTerminal},
    systemd_units_journal::graphical_session_target::{DesktopName, GraphicalSession, Xwayland},
};
use sottomano_widgets::{
    clock::Clock,
    indicator_groups::login_session_group::{
        LoginSessionGroup, LoginSessionInteraction, LoginSessionMessage,
    },
};

use crate::showcased_group::ShowcasedGroup;

pub fn group(clock: Clock) -> ShowcasedGroup {
    ShowcasedGroup::LoginSession(Box::new(session_indicators().into_iter().fold(
        LoginSessionGroup::default(),
        |mut group, fixture_message| {
            let _ = group.update(fixture_message, clock);
            group
        },
    )))
}

fn session_indicators() -> [LoginSessionMessage; 2] {
    [
        LoginSessionMessage::SystemFetched(SnapshotDelivery::live_push(Ok(
            OperatingSystemIdentity {
                hostname: "workstation".to_owned(),
                kernel: "6.18.2".to_owned(),
                operating_system: "NixOS 26.05 (Yarara)".to_owned(),
                distribution_ids: vec!["nixos".to_owned()],
                chassis: Chassis::Desktop,
            },
        ))),
        LoginSessionMessage::IdentityFetched(SnapshotDelivery::live_push(Ok(SessionIdentity {
            user: "demo".to_owned(),
            uid: 1000,
            seat: "seat0".to_owned(),
            virtual_terminal: VirtualTerminal::Console(
                NonZeroU32::new(2).expect("the showcase console number is not zero"),
            ),
            login_manager: "greetd".to_owned(),
            desktop: DesktopName::Named("niri".to_owned()),
            session_type: "wayland".to_owned(),
        }))),
    ]
}

pub fn on_hover(started_fetch: &LoginSessionInteraction) -> Vec<LoginSessionMessage> {
    match started_fetch {
        LoginSessionInteraction::StartCompositorFetch => {
            vec![LoginSessionMessage::GraphicalFetched(
                SnapshotDelivery::single_read(Ok(niri_graphical_session())),
            )]
        }
        _ => Vec::new(),
    }
}

fn niri_graphical_session() -> GraphicalSession {
    GraphicalSession {
        current_desktop: DesktopName::Named("niri".to_owned()),
        session_desktop: DesktopName::Named("niri".to_owned()),
        compositor_version: "niri 25.11".to_owned(),
        wayland_display: "wayland-1".to_owned(),
        x11_display: Xwayland::Serving(":0".to_owned()),
    }
}
