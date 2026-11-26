#[cfg(test)]
mod focused_menu_bar_tests;
mod menu_bar_candidate_discovery;

use std::collections::HashMap;

use futures::{Stream, StreamExt, TryStreamExt, stream, stream::BoxStream};
use futures_rx::RxExt;
use sottomano_compositor_model::{
    compositor_window_facts::{WindowAppId, WindowPid},
    compositor_window_state::CompositorWindow,
};

use self::menu_bar_candidate_discovery::{
    MenuBarCandidate, WindowProtocol, native_menu_bar_candidate, window_protocol_of_owner,
    x11_menu_bar_candidate,
};
use crate::{
    desktop_windows_apps::{
        application_menu_nodes::MenuNode,
        appmenu_registrar_server::RegisteredMenuBar,
        dbusmenu_menu_bar_layout::{DbusMenuLocation, menu_bar_layout_updates},
        gtk_menus_menu_bar::{
            GtkActionGroups, GtkMenuActivation, GtkMenuBar, GtkMenuBarSource,
            gtk_menu_bar_source_of_app, gtk_menu_bar_updates,
        },
        x11_window_menu_properties::{X11MenuLookupError, active_x11_window_changes},
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

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
        let process_id = match compositor_window.pid {
            WindowPid::Reported(window_pid) => u32::try_from(window_pid)
                .inspect_err(|pid_failure| {
                    tracing::warn!("global menu: invalid window pid {window_pid}: {pid_failure}");
                })
                .ok(),
            WindowPid::Unreported => None,
        };
        let window_app_id = compositor_window.app_id.clone();
        match process_id {
            Some(process_id) => match window_app_id {
                WindowAppId::Reported(app_id) => Self::ProcessAndApp { process_id, app_id },
                WindowAppId::Unreported => Self::ProcessOnly { process_id },
            },
            None => match window_app_id {
                WindowAppId::Reported(app_id) => Self::AppOnly { app_id },
                WindowAppId::Unreported => Self::Unidentified,
            },
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
