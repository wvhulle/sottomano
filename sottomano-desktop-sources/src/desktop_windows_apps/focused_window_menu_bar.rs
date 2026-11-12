use std::collections::HashMap;

use futures::{Stream, StreamExt, TryStreamExt, stream, stream::BoxStream};
use futures_rx::RxExt;
use tokio::fs;

use crate::{
    compositor_window_ipc::compositor_window_state::CompositorWindow,
    desktop_windows_apps::{
        application_menu_nodes::MenuNode,
        appmenu_registrar_server::RegisteredMenuBar,
        dbusmenu_menu_bar_layout::{
            DbusMenuLocation, exported_menu_bars_of_process, menu_bar_layout_updates,
        },
        gtk_menus_menu_bar::{
            GtkActionGroups, GtkMenuActivation, GtkMenuBar, GtkMenuBarSource,
            gtk_menu_bar_source_of_app, gtk_menu_bar_updates,
        },
        x11_window_menu_properties::{
            ActiveX11Window, X11MenuLookupError, X11WindowMenu, active_x11_window_changes,
            x11_window_menu,
        },
    },
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, OwnerProcess},
};

const X11_BRIDGE_EXECUTABLES: [&str; 2] = ["xwayland-satellite", "Xwayland"];

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FocusedWindowOwner {
    ProcessAndApp { process_id: u32, app_id: String },
    ProcessOnly { process_id: u32 },
    AppOnly { app_id: String },
    Unidentified,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FocusedWindowIdentity {
    Window {
        window_id: u64,
        owner: FocusedWindowOwner,
    },
    NoFocusedWindow,
}

impl From<&CompositorWindow> for FocusedWindowOwner {
    fn from(compositor_window: &CompositorWindow) -> Self {
        let process_id = compositor_window.pid.and_then(|window_pid| {
            u32::try_from(window_pid)
                .inspect_err(|pid_failure| {
                    tracing::warn!("global menu: invalid window pid {window_pid}: {pid_failure}");
                })
                .ok()
        });
        match (process_id, compositor_window.app_id.clone()) {
            (Some(process_id), Some(app_id)) => Self::ProcessAndApp { process_id, app_id },
            (Some(process_id), None) => Self::ProcessOnly { process_id },
            (None, Some(app_id)) => Self::AppOnly { app_id },
            (None, None) => Self::Unidentified,
        }
    }
}

impl From<Option<&CompositorWindow>> for FocusedWindowIdentity {
    fn from(focused_window: Option<&CompositorWindow>) -> Self {
        match focused_window {
            Some(focused_window) => Self::Window {
                window_id: focused_window.id,
                owner: FocusedWindowOwner::from(focused_window),
            },
            None => Self::NoFocusedWindow,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MenuBarQuery {
    pub buses: DesktopBuses,
    pub focused_window: FocusedWindowIdentity,
    pub registered_menu_bars: Vec<RegisteredMenuBar>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuBarMatch {
    ExactWindow,
    OnlyProcessCandidate,
    MostRecentProcessCandidate,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MenuActivationRoute {
    DbusMenu(DbusMenuLocation),
    GtkActions {
        action_groups: GtkActionGroups,
        activations: HashMap<i32, GtkMenuActivation>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShownMenuBar {
    pub top_menus: Vec<MenuNode>,
    pub activation_route: MenuActivationRoute,
    pub menu_bar_match: MenuBarMatch,
}

#[derive(Debug, PartialEq)]
pub enum FocusedMenuBar {
    Shown(ShownMenuBar),
    NotExported,
    NoFocusedWindow,
    LookupFailed(X11MenuLookupError),
}

fn shown_gtk_menu_bar(gtk_menu_bar: GtkMenuBar, menu_bar_match: MenuBarMatch) -> ShownMenuBar {
    ShownMenuBar {
        top_menus: gtk_menu_bar.top_menus,
        activation_route: MenuActivationRoute::GtkActions {
            action_groups: gtk_menu_bar.action_groups,
            activations: gtk_menu_bar.activations,
        },
        menu_bar_match,
    }
}

enum DbusMenuCandidate {
    Exported {
        location: DbusMenuLocation,
        menu_bar_match: MenuBarMatch,
    },
    NoneExported,
}

enum MenuBarCandidate {
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
enum WindowProtocol {
    X11Bridged,
    Native,
}

fn newest_dbusmenu_candidate(
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

fn window_protocol_of_executable(executable_name: &str) -> WindowProtocol {
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

fn registered_for_x11_window(
    registered_menu_bars: &[RegisteredMenuBar],
    x11_window_id: u32,
) -> Option<DbusMenuLocation> {
    registered_menu_bars
        .iter()
        .rev()
        .find(|registered| registered.window_id == x11_window_id)
        .map(DbusMenuLocation::from)
}

async fn x11_menu_bar_candidate(
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

async fn native_menu_bar_candidate(
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

async fn window_protocol_of_owner(owner: &FocusedWindowOwner) -> WindowProtocol {
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

fn shown_or_not_exported(shown_menu_bar: ShownMenuBar) -> FocusedMenuBar {
    if shown_menu_bar.top_menus.is_empty() {
        FocusedMenuBar::NotExported
    } else {
        FocusedMenuBar::Shown(shown_menu_bar)
    }
}

fn dbusmenu_menu_bar_stream(
    connection: zbus::Connection,
    location: DbusMenuLocation,
    menu_bar_match: MenuBarMatch,
) -> impl Stream<Item = FocusedMenuBar> + Send {
    let logged_location = location.clone();
    stream::once(menu_bar_layout_updates(connection, location.clone()))
        .map_ok(move |layout_updates| {
            let location = location.clone();
            layout_updates
                .map(move |top_menus| match top_menus {
                    Ok(top_menus) => shown_or_not_exported(ShownMenuBar {
                        top_menus,
                        activation_route: MenuActivationRoute::DbusMenu(location.clone()),
                        menu_bar_match,
                    }),
                    Err(layout_failure) => {
                        tracing::debug!("global menu: dbusmenu layout: {layout_failure}");
                        FocusedMenuBar::NotExported
                    }
                })
                .map(Ok::<_, zbus::Error>)
        })
        .try_flatten()
        .map(move |menu_bar| {
            menu_bar.unwrap_or_else(|connect_failure| {
                tracing::debug!("global menu: dbusmenu {logged_location:?}: {connect_failure}");
                FocusedMenuBar::NotExported
            })
        })
}

fn gtk_menu_bar_stream(
    connection: zbus::Connection,
    source: GtkMenuBarSource,
    menu_bar_match: MenuBarMatch,
) -> impl Stream<Item = FocusedMenuBar> + Send {
    stream::once(gtk_menu_bar_updates(connection, source))
        .map_ok(move |menu_bar_updates| {
            menu_bar_updates
                .map(move |gtk_menu_bar| match gtk_menu_bar {
                    Ok(gtk_menu_bar) => {
                        shown_or_not_exported(shown_gtk_menu_bar(gtk_menu_bar, menu_bar_match))
                    }
                    Err(menus_failure) => {
                        tracing::debug!("global menu: gtk menus: {menus_failure}");
                        FocusedMenuBar::NotExported
                    }
                })
                .map(Ok::<_, zbus::Error>)
        })
        .try_flatten()
        .map(|menu_bar| {
            menu_bar.unwrap_or_else(|connect_failure| {
                tracing::debug!("global menu: no gtk menu bar: {connect_failure}");
                FocusedMenuBar::NotExported
            })
        })
}

fn gtk_application_stream(
    connection: zbus::Connection,
    app_id: String,
) -> impl Stream<Item = FocusedMenuBar> + Send {
    let logged_app_id = app_id.clone();
    stream::once(async move {
        gtk_menu_bar_source_of_app(&connection, &app_id)
            .await
            .map(|source| {
                gtk_menu_bar_stream(connection, source, MenuBarMatch::OnlyProcessCandidate)
                    .map(Ok::<_, zbus::Error>)
            })
    })
    .try_flatten()
    .map(move |menu_bar| {
        menu_bar.unwrap_or_else(|source_failure| {
            tracing::debug!(
                "global menu: {logged_app_id} is no gtk application name: {source_failure}"
            );
            FocusedMenuBar::NotExported
        })
    })
}

fn candidate_menu_bars(
    connection: zbus::Connection,
    owner: &FocusedWindowOwner,
    candidate: MenuBarCandidate,
) -> BoxStream<'static, FocusedMenuBar> {
    match candidate {
        MenuBarCandidate::DbusMenu {
            location,
            menu_bar_match,
        } => {
            tracing::debug!("global menu: {owner:?} exports {location:?} ({menu_bar_match:?})");
            dbusmenu_menu_bar_stream(connection, location, menu_bar_match).boxed()
        }
        MenuBarCandidate::GtkMenus {
            source,
            menu_bar_match,
        } => {
            tracing::debug!("global menu: {owner:?} exports {source:?} ({menu_bar_match:?})");
            gtk_menu_bar_stream(connection, source, menu_bar_match).boxed()
        }
        MenuBarCandidate::GtkApplication { app_id } => {
            gtk_application_stream(connection, app_id).boxed()
        }
        MenuBarCandidate::Absent => {
            tracing::debug!("global menu: {owner:?} exports no menu bar");
            stream::iter([FocusedMenuBar::NotExported]).boxed()
        }
    }
}

fn x11_window_menu_bars(
    connection: zbus::Connection,
    owner: FocusedWindowOwner,
    registered_menu_bars: Vec<RegisteredMenuBar>,
) -> BoxStream<'static, FocusedMenuBar> {
    active_x11_window_changes()
        .switch_map(move |active_window| {
            let (lookup_failures, active_windows) = match active_window {
                Ok(active_window) => (Vec::new(), vec![active_window]),
                Err(lookup_failure) => (
                    vec![FocusedMenuBar::LookupFailed(lookup_failure)],
                    Vec::new(),
                ),
            };
            let connection = connection.clone();
            let owner = owner.clone();
            let registered_menu_bars = registered_menu_bars.clone();
            stream::iter(lookup_failures).chain(stream::iter(active_windows).flat_map(
                move |active_window| {
                    let connection = connection.clone();
                    let owner = owner.clone();
                    let registered_menu_bars = registered_menu_bars.clone();
                    stream::once(async move {
                        let candidate =
                            x11_menu_bar_candidate(&registered_menu_bars, active_window).await;
                        candidate_menu_bars(connection, &owner, candidate)
                    })
                    .flatten()
                },
            ))
        })
        .boxed()
}

pub fn focused_menu_bar_updates(
    query: &MenuBarQuery,
) -> impl Stream<Item = FocusedMenuBar> + Send + use<> {
    let query = query.clone();
    stream::once(async move {
        let connection = query.buses.session.clone();
        let FocusedWindowIdentity::Window { owner, .. } = &query.focused_window else {
            return stream::iter([FocusedMenuBar::NoFocusedWindow]).boxed();
        };
        match window_protocol_of_owner(owner).await {
            WindowProtocol::X11Bridged => x11_window_menu_bars(
                connection,
                owner.clone(),
                query.registered_menu_bars.clone(),
            ),
            WindowProtocol::Native => {
                let candidate =
                    native_menu_bar_candidate(&connection, owner, &query.registered_menu_bars)
                        .await;
                candidate_menu_bars(connection, owner, candidate)
            }
        }
    })
    .flatten()
}

#[cfg(test)]
mod tests {
    use zbus::{
        names::{OwnedBusName, OwnedUniqueName},
        zvariant::OwnedObjectPath,
    };

    use super::*;
    use crate::compositor_window_ipc::compositor_window_state::CompositorWindowLayout;

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
        let candidate =
            newest_dbusmenu_candidate(Vec::new(), vec![location(":1.40", "/MenuBar/1")]);
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
            title: None,
            app_id: Some("org.example.Editor".to_owned()),
            pid: None,
            workspace_id: None,
            is_focused: true,
            is_floating: false,
            is_urgent: false,
            layout: CompositorWindowLayout::default(),
            focused_at_monotonic: None,
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
}
