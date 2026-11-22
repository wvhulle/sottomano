use tokio::fs;

use super::{FocusedWindowOwner, MenuBarMatch};
use crate::{
    desktop_windows_apps::{
        appmenu_registrar_server::RegisteredMenuBar,
        dbusmenu_menu_bar_layout::{DbusMenuLocation, exported_menu_bars_of_process},
        gtk_menus_menu_bar::GtkMenuBarSource,
        x11_window_menu_properties::{ActiveX11Window, X11WindowMenu, x11_window_menu},
    },
    system_protocol_clients::dbus_desktop_buses::OwnerProcess,
};

const X11_BRIDGE_EXECUTABLES: [&str; 2] = ["xwayland-satellite", "Xwayland"];

pub(super) enum DbusMenuCandidate {
    Exported {
        location: DbusMenuLocation,
        menu_bar_match: MenuBarMatch,
    },
    NoneExported,
}

pub(super) enum MenuBarCandidate {
    DbusMenu {
        location: DbusMenuLocation,
        menu_bar_match: MenuBarMatch,
    },
    GtkMenus {
        source: GtkMenuBarSource,
        menu_bar_match: MenuBarMatch,
    },
    GtkApplication {
        app_id: String,
    },
    Absent,
}

impl From<DbusMenuCandidate> for MenuBarCandidate {
    fn from(dbusmenu_candidate: DbusMenuCandidate) -> Self {
        match dbusmenu_candidate {
            DbusMenuCandidate::Exported {
                location,
                menu_bar_match,
            } => Self::DbusMenu {
                location,
                menu_bar_match,
            },
            DbusMenuCandidate::NoneExported => Self::Absent,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WindowProtocol {
    X11Bridged,
    Native,
}

pub(super) fn newest_dbusmenu_candidate(
    registered_locations: Vec<DbusMenuLocation>,
    exported_locations: Vec<DbusMenuLocation>,
) -> DbusMenuCandidate {
    let mut candidate_locations = exported_locations;
    for registered_location in registered_locations {
        candidate_locations.retain(|candidate| *candidate != registered_location);
        candidate_locations.push(registered_location);
    }
    let menu_bar_match = if candidate_locations.len() > 1 {
        MenuBarMatch::MostRecentProcessCandidate
    } else {
        MenuBarMatch::OnlyProcessCandidate
    };
    match candidate_locations.pop() {
        Some(location) => DbusMenuCandidate::Exported {
            location,
            menu_bar_match,
        },
        None => DbusMenuCandidate::NoneExported,
    }
}

async fn dbusmenu_candidate_of_process(
    connection: &zbus::Connection,
    process_id: u32,
    registered_menu_bars: &[RegisteredMenuBar],
) -> DbusMenuCandidate {
    let registered_locations = registered_menu_bars
        .iter()
        .filter(|registered| registered.owner_process == OwnerProcess::Identified(process_id))
        .map(DbusMenuLocation::from)
        .collect();
    let exported_locations = exported_menu_bars_of_process(connection, process_id)
        .await
        .unwrap_or_else(|probe_failure| {
            tracing::debug!("global menu: probing process {process_id}: {probe_failure}");
            Vec::new()
        });
    newest_dbusmenu_candidate(registered_locations, exported_locations)
}

pub(super) fn window_protocol_of_executable(executable_name: &str) -> WindowProtocol {
    let unwrapped_name = executable_name
        .trim_start_matches('.')
        .trim_end_matches("-wrapped");
    if X11_BRIDGE_EXECUTABLES.contains(&unwrapped_name) {
        WindowProtocol::X11Bridged
    } else {
        WindowProtocol::Native
    }
}

async fn window_protocol_of_process(process_id: u32) -> WindowProtocol {
    let Ok(executable_path) = fs::read_link(format!("/proc/{process_id}/exe")).await else {
        return WindowProtocol::Native;
    };
    executable_path
        .file_name()
        .and_then(|executable_name| executable_name.to_str())
        .map_or(WindowProtocol::Native, window_protocol_of_executable)
}

pub(super) fn registered_for_x11_window(
    registered_menu_bars: &[RegisteredMenuBar],
    x11_window_id: u32,
) -> Option<DbusMenuLocation> {
    registered_menu_bars
        .iter()
        .rev()
        .find(|registered| registered.window_id == x11_window_id)
        .map(DbusMenuLocation::from)
}

pub(super) async fn x11_menu_bar_candidate(
    registered_menu_bars: &[RegisteredMenuBar],
    active_window: ActiveX11Window,
) -> MenuBarCandidate {
    let ActiveX11Window::Window(x11_window_id) = active_window else {
        return MenuBarCandidate::Absent;
    };
    if let Some(location) = registered_for_x11_window(registered_menu_bars, x11_window_id) {
        return MenuBarCandidate::DbusMenu {
            location,
            menu_bar_match: MenuBarMatch::ExactWindow,
        };
    }
    match x11_window_menu(x11_window_id).await {
        Ok(X11WindowMenu::KdeDbusMenu(location)) => MenuBarCandidate::DbusMenu {
            location,
            menu_bar_match: MenuBarMatch::ExactWindow,
        },
        Ok(X11WindowMenu::GtkMenus(source)) => MenuBarCandidate::GtkMenus {
            source,
            menu_bar_match: MenuBarMatch::ExactWindow,
        },
        Ok(X11WindowMenu::Unpublished) => MenuBarCandidate::Absent,
        Err(lookup_failure) => {
            tracing::debug!("global menu: x11 window {x11_window_id:#x}: {lookup_failure}");
            MenuBarCandidate::Absent
        }
    }
}

pub(super) async fn native_menu_bar_candidate(
    connection: &zbus::Connection,
    owner: &FocusedWindowOwner,
    registered_menu_bars: &[RegisteredMenuBar],
) -> MenuBarCandidate {
    match owner {
        FocusedWindowOwner::ProcessAndApp { process_id, app_id } => {
            match dbusmenu_candidate_of_process(connection, *process_id, registered_menu_bars).await
            {
                DbusMenuCandidate::NoneExported => MenuBarCandidate::GtkApplication {
                    app_id: app_id.clone(),
                },
                exported @ DbusMenuCandidate::Exported { .. } => MenuBarCandidate::from(exported),
            }
        }
        FocusedWindowOwner::ProcessOnly { process_id } => MenuBarCandidate::from(
            dbusmenu_candidate_of_process(connection, *process_id, registered_menu_bars).await,
        ),
        FocusedWindowOwner::AppOnly { app_id } => MenuBarCandidate::GtkApplication {
            app_id: app_id.clone(),
        },
        FocusedWindowOwner::Unidentified => MenuBarCandidate::Absent,
    }
}

pub(super) async fn window_protocol_of_owner(owner: &FocusedWindowOwner) -> WindowProtocol {
    match owner {
        FocusedWindowOwner::ProcessAndApp { process_id, .. }
        | FocusedWindowOwner::ProcessOnly { process_id } => {
            window_protocol_of_process(*process_id).await
        }
        FocusedWindowOwner::AppOnly { .. } | FocusedWindowOwner::Unidentified => {
            WindowProtocol::Native
        }
    }
}
