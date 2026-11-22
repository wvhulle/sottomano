use sottomano_compositor_model::{
    compositor_window_facts::{
        FocusTimestamp, Urgency, WindowAppId, WindowFocus, WindowPid, WindowTiling, WindowTitle,
        WindowWorkspace,
    },
    compositor_window_state::CompositorWindowLayout,
};
use zbus::{
    names::{OwnedBusName, OwnedUniqueName},
    zvariant::OwnedObjectPath,
};

use super::{
    menu_bar_candidate_discovery::{
        DbusMenuCandidate, newest_dbusmenu_candidate, registered_for_x11_window,
        window_protocol_of_executable,
    },
    *,
};
use crate::system_protocol_clients::dbus_desktop_buses::OwnerProcess;

fn location(bus_name: &str, menu_path: &str) -> DbusMenuLocation {
    DbusMenuLocation {
        bus_name: OwnedBusName::try_from(bus_name).expect("bus name"),
        menu_path: OwnedObjectPath::try_from(menu_path).expect("object path"),
    }
}

#[test]
fn registered_menu_bar_wins_over_probed_ones() {
    let candidate = newest_dbusmenu_candidate(
        vec![location(":1.40", "/MenuBar/1")],
        vec![
            location(":1.40", "/MenuBar/1"),
            location(":1.40", "/MenuBar/2"),
        ],
    );
    let DbusMenuCandidate::Exported {
        location: chosen_location,
        menu_bar_match,
    } = candidate
    else {
        panic!("a dbusmenu candidate is expected");
    };
    assert_eq!(chosen_location, location(":1.40", "/MenuBar/1"));
    assert_eq!(menu_bar_match, MenuBarMatch::MostRecentProcessCandidate);
}

#[test]
fn single_probed_menu_bar_is_the_only_candidate() {
    let candidate = newest_dbusmenu_candidate(Vec::new(), vec![location(":1.40", "/MenuBar/1")]);
    assert!(matches!(
        candidate,
        DbusMenuCandidate::Exported {
            menu_bar_match: MenuBarMatch::OnlyProcessCandidate,
            ..
        }
    ));
}

#[test]
fn process_without_menu_bars_has_no_dbusmenu_candidate() {
    assert!(matches!(
        newest_dbusmenu_candidate(Vec::new(), Vec::new()),
        DbusMenuCandidate::NoneExported
    ));
}

#[test]
fn window_without_pid_is_identified_by_its_app() {
    let focused_window = CompositorWindow {
        id: 7,
        title: WindowTitle::Untitled,
        app_id: WindowAppId::Reported("org.example.Editor".to_owned()),
        pid: WindowPid::Unreported,
        workspace: WindowWorkspace::Unplaced,
        focus: WindowFocus::Focused,
        tiling: WindowTiling::Tiled,
        urgency: Urgency::NotUrgent,
        layout: CompositorWindowLayout::default(),
        focused_at_monotonic: FocusTimestamp::NeverFocused,
    };
    assert_eq!(
        FocusedWindowIdentity::from(Some(&focused_window)),
        FocusedWindowIdentity::Window {
            window_id: 7,
            owner: FocusedWindowOwner::AppOnly {
                app_id: "org.example.Editor".to_owned()
            },
        }
    );
}

#[test]
fn latest_registration_of_an_x11_window_wins() {
    let registration = |menu_path: &str| RegisteredMenuBar {
        window_id: 0x0060_0003,
        owner_bus_name: OwnedUniqueName::try_from(":1.70").expect("unique name"),
        owner_process: OwnerProcess::Identified(4242),
        menu_path: OwnedObjectPath::try_from(menu_path).expect("object path"),
    };
    let registered = [
        registration("/com/canonical/menu/600003"),
        registration("/com/canonical/menu/600003/refreshed"),
    ];
    assert_eq!(
        registered_for_x11_window(&registered, 0x0060_0003),
        Some(location(":1.70", "/com/canonical/menu/600003/refreshed"))
    );
    assert_eq!(registered_for_x11_window(&registered, 0x0070_0001), None);
}

#[test]
fn nix_wrapped_satellite_bridges_x11_windows() {
    assert_eq!(
        window_protocol_of_executable(".xwayland-satellite-wrapped"),
        WindowProtocol::X11Bridged
    );
    assert_eq!(
        window_protocol_of_executable("Xwayland"),
        WindowProtocol::X11Bridged
    );
    assert_eq!(
        window_protocol_of_executable("dolphin"),
        WindowProtocol::Native
    );
}
