use std::collections::HashSet;

use iced::{
    Element, Subscription,
    border::Radius,
    widget::{mouse_area, text},
};
use sottomano_desktop_sinks::desktop_windows_apps::{
    dbusmenu_item_events::{click_dbusmenu_item, open_dbusmenu_submenu},
    gtk_action_activation::activate_gtk_action,
};
use sottomano_desktop_sources::{
    desktop_windows_apps::{
        application_menu_nodes::MenuNode,
        appmenu_registrar_server::{self, RegisteredMenuBar},
        dbusmenu_menu_bar_layout::DbusMenuLocation,
        focused_window_menu_bar::{
            FocusedMenuBar, FocusedWindowIdentity, MenuActivationRoute, MenuBarMatch, MenuBarQuery,
            ShownMenuBar, focused_menu_bar_updates,
        },
        gtk_menus_menu_bar::{GtkActionGroups, GtkMenuActivation},
    },
    stream_operators::restarting,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::{Config, widget_groups::AppMenuRegistrarClaim};

use crate::{
    group_items::{
        group_item::{
            ComposedGroupItemFace, GroupItem, GroupItemHoverArea, GroupItemIcon, GroupItemValue,
        },
        reading_display_state::{ReadingDisplayState, ReadingStateTint},
    },
    hover_cards::{
        HoverTarget, PressHints,
        hover_card_body::{
            HoverCardBody, Section,
            hover_card_source_delivery::{DeliveryTiming, HoverCard},
        },
        hoverable,
    },
    widget_groups::{
        widget_group_content::{
            GroupEvent, WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
            WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        application_menu_lines::ApplicationMenuLines,
        context_menu::context_menu,
        icon_handle::PixmapHandles,
        phosphor::{Phosphor, Weight},
        text::panel_text,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalMenuTarget {
    TopMenu(i32),
}

impl From<GlobalMenuTarget> for HoverTarget {
    fn from(target: GlobalMenuTarget) -> Self {
        HoverTarget::GlobalMenu(target)
    }
}

pub struct GlobalMenuGroup {
    registered_menu_bars: Vec<RegisteredMenuBar>,
    focused_window: FocusedWindowIdentity,
    focused_menu_bar: FocusedMenuBar,
    pixmap_handles: PixmapHandles,
    open_submenus: HashSet<i32>,
    expansion: WidgetGroupExpansion,
}

impl Default for GlobalMenuGroup {
    fn default() -> Self {
        Self {
            registered_menu_bars: Vec::new(),
            focused_window: FocusedWindowIdentity::NoFocusedWindow,
            focused_menu_bar: FocusedMenuBar::NoFocusedWindow,
            pixmap_handles: PixmapHandles::default(),
            open_submenus: HashSet::new(),
            expansion: WidgetGroupExpansion::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum GlobalMenuMessage {
    RegistrationsChanged(Vec<RegisteredMenuBar>),
    MenuBarChanged(FocusedMenuBar),
    TopMenuEntered(i32),
    ItemPressed(i32),
    ToggleSubmenu(i32),
    Expansion(WidgetGroupExpansionMessage),
}

#[derive(Debug, Clone)]
pub enum GlobalMenuRequest {
    OpenDbusMenuSubmenu {
        location: DbusMenuLocation,
        item_id: i32,
    },
    ClickDbusMenuItem {
        location: DbusMenuLocation,
        item_id: i32,
    },
    ActivateGtkAction {
        action_groups: GtkActionGroups,
        activation: GtkMenuActivation,
    },
}

impl GlobalMenuRequest {
    pub async fn send(self, session_bus: zbus::Connection) {
        match self {
            GlobalMenuRequest::OpenDbusMenuSubmenu { location, item_id } => {
                open_dbusmenu_submenu(session_bus, location, item_id).await;
            }
            GlobalMenuRequest::ClickDbusMenuItem { location, item_id } => {
                click_dbusmenu_item(session_bus, location, item_id).await;
            }
            GlobalMenuRequest::ActivateGtkAction {
                action_groups,
                activation,
            } => activate_gtk_action(session_bus, action_groups, activation).await,
        }
    }
}

pub enum Action {
    None,
    Prepare(GlobalMenuRequest),
    Activate(GlobalMenuRequest),
}

const MOST_RECENT_OF_SEVERAL_NOTE: &str =
    "This application exports several menu bars; the most recently created one is shown.";

impl GlobalMenuGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn observe_focused_window(&mut self, focused_window: FocusedWindowIdentity) {
        if focused_window == self.focused_window {
            return;
        }
        self.focused_window = focused_window;
        self.focused_menu_bar = FocusedMenuBar::NotExported;
        self.open_submenus.clear();
    }

    pub fn subscription(
        &self,
        buses: &DesktopBuses,
        registrar_claim: AppMenuRegistrarClaim,
    ) -> Subscription<GlobalMenuMessage> {
        let registrar = match registrar_claim {
            AppMenuRegistrarClaim::Claimed => Subscription::run(|| {
                restarting("appmenu registrar", appmenu_registrar_server::serve)
            })
            .map(GlobalMenuMessage::RegistrationsChanged),
            AppMenuRegistrarClaim::Left => Subscription::none(),
        };
        let menu_bar_query = MenuBarQuery {
            buses: buses.clone(),
            focused_window: self.focused_window.clone(),
            registered_menu_bars: self.registered_menu_bars.clone(),
        };
        Subscription::batch([
            registrar,
            Subscription::run_with(menu_bar_query, focused_menu_bar_updates)
                .map(GlobalMenuMessage::MenuBarChanged),
        ])
    }

    pub fn update(&mut self, message: GlobalMenuMessage) -> Action {
        match message {
            GlobalMenuMessage::RegistrationsChanged(registered_menu_bars) => {
                self.registered_menu_bars = registered_menu_bars;
                Action::None
            }
            GlobalMenuMessage::MenuBarChanged(focused_menu_bar) => {
                if let FocusedMenuBar::Shown(shown_menu_bar) = &focused_menu_bar {
                    self.pixmap_handles = self
                        .pixmap_handles
                        .refreshed(shown_menu_bar.top_menus.iter().flat_map(MenuNode::pixmaps));
                }
                self.focused_menu_bar = focused_menu_bar;
                Action::None
            }
            GlobalMenuMessage::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Action::None
            }
            GlobalMenuMessage::ToggleSubmenu(item_id) => {
                if !self.open_submenus.remove(&item_id) {
                    self.open_submenus.insert(item_id);
                }
                Action::None
            }
            GlobalMenuMessage::TopMenuEntered(item_id) => match &self.focused_menu_bar {
                FocusedMenuBar::Shown(ShownMenuBar {
                    activation_route: MenuActivationRoute::DbusMenu(location),
                    ..
                }) => Action::Prepare(GlobalMenuRequest::OpenDbusMenuSubmenu {
                    location: location.clone(),
                    item_id,
                }),
                FocusedMenuBar::Shown(_)
                | FocusedMenuBar::NotExported
                | FocusedMenuBar::NoFocusedWindow => Action::None,
            },
            GlobalMenuMessage::ItemPressed(item_id) => {
                self.open_submenus.clear();
                self.item_activation(item_id)
            }
        }
    }

    fn item_activation(&self, item_id: i32) -> Action {
        let FocusedMenuBar::Shown(shown_menu_bar) = &self.focused_menu_bar else {
            return Action::None;
        };
        match &shown_menu_bar.activation_route {
            MenuActivationRoute::DbusMenu(location) => {
                Action::Activate(GlobalMenuRequest::ClickDbusMenuItem {
                    location: location.clone(),
                    item_id,
                })
            }
            MenuActivationRoute::GtkActions {
                action_groups,
                activations,
            } => match activations.get(&item_id) {
                Some(activation) => Action::Activate(GlobalMenuRequest::ActivateGtkAction {
                    action_groups: action_groups.clone(),
                    activation: activation.clone(),
                }),
                None => Action::None,
            },
        }
    }

    fn top_menu(&self, item_id: i32) -> Option<&MenuNode> {
        match &self.focused_menu_bar {
            FocusedMenuBar::Shown(shown_menu_bar) => shown_menu_bar
                .top_menus
                .iter()
                .find(|top_menu| top_menu.id == item_id),
            FocusedMenuBar::NotExported | FocusedMenuBar::NoFocusedWindow => None,
        }
    }

    #[must_use]
    pub fn hover_card_for(&self, target: GlobalMenuTarget) -> Option<HoverCard> {
        let GlobalMenuTarget::TopMenu(item_id) = target;
        let FocusedMenuBar::Shown(shown_menu_bar) = &self.focused_menu_bar else {
            return None;
        };
        self.top_menu(item_id)?;
        let body = match shown_menu_bar.menu_bar_match {
            MenuBarMatch::ExactWindow | MenuBarMatch::OnlyProcessCandidate => {
                HoverCardBody::sections([])
            }
            MenuBarMatch::MostRecentProcessCandidate => {
                HoverCardBody::from(Section::note(MOST_RECENT_OF_SEVERAL_NOTE.to_owned()))
            }
        };
        Some(HoverCard::sourced(body, DeliveryTiming::LivePush))
    }

    #[must_use]
    pub fn menu_panel<'a>(
        &'a self,
        target: GlobalMenuTarget,
        config: &'a Config,
        popup_corners: Radius,
    ) -> Option<Element<'a, GlobalMenuMessage>> {
        let GlobalMenuTarget::TopMenu(item_id) = target;
        let top_menu = self.top_menu(item_id)?;
        let lines = ApplicationMenuLines {
            config,
            pixmap_handles: &self.pixmap_handles,
            open: &self.open_submenus,
            on_activate: &GlobalMenuMessage::ItemPressed,
            on_toggle: &GlobalMenuMessage::ToggleSubmenu,
        }
        .lines(&top_menu.children);
        Some(context_menu(config, popup_corners, lines))
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
    ) -> WidgetGroupContent<'a, GlobalMenuMessage> {
        let top_menus: &[MenuNode] = match &self.focused_menu_bar {
            FocusedMenuBar::Shown(shown_menu_bar) => &shown_menu_bar.top_menus,
            FocusedMenuBar::NotExported | FocusedMenuBar::NoFocusedWindow => &[],
        };
        let group_items = top_menus.iter().filter_map(|top_menu| {
            let label = top_menu.label.clone()?;
            let target = GlobalMenuTarget::TopMenu(top_menu.id);
            let item_id = top_menu.id;
            Some(GroupItem::composed(
                ComposedGroupItemFace {
                    name: label.clone().into(),
                    icon: GroupItemIcon::Absent,
                },
                GroupItemHoverArea::ByChildren {
                    row_target: target.into(),
                },
                GroupItemValue::drawn(move |reveal| {
                    top_menu_view(config, label.clone(), target, item_id, reveal)
                }),
            ))
        });
        WidgetGroupContent::expandable(
            WidgetGroupFamily::GlobalMenu,
            &self.expansion,
            GlobalMenuMessage::Expansion,
            group_items.collect::<Vec<_>>(),
        )
        .face(WidgetGroupFace::Icon {
            glyph: Phosphor::ListDashes(Weight::Regular).into(),
        })
    }
}

fn top_menu_view<'a>(
    config: &Config,
    label: String,
    target: GlobalMenuTarget,
    item_id: i32,
    reveal: f32,
) -> Element<'a, GroupEvent<GlobalMenuMessage>> {
    let tint = ReadingStateTint {
        state: ReadingDisplayState::Idle,
        opacity: reveal,
    };
    let label_text =
        panel_text(&config.theme.font, label).style(move |theme: &iced::Theme| text::Style {
            color: Some(tint.color(theme)),
        });
    let hover = hoverable(
        label_text.into(),
        target.into(),
        PressHints::default(),
        GroupEvent::HoverCard,
    );
    mouse_area(hover)
        .on_enter(GroupEvent::Group(GlobalMenuMessage::TopMenuEntered(
            item_id,
        )))
        .into()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use iced_test::Simulator;
    use sottomano_desktop_sources::desktop_windows_apps::{
        application_menu_nodes::MenuKind,
        focused_window_menu_bar::FocusedWindowOwner,
        gtk_menus_menu_bar::{
            ApplicationActionPath, GtkActionScope, GtkActionTarget, WindowActionPath,
        },
    };
    use sottomano_toml_config::screen_edges::Position;
    use zbus::{names::OwnedBusName, zvariant::OwnedObjectPath};

    use super::*;
    use crate::clock::Clock;

    const FILE_MENU_ID: i32 = 1;
    const QUIT_ITEM_ID: i32 = 2;

    fn file_menu_with_quit() -> Vec<MenuNode> {
        vec![MenuNode {
            id: FILE_MENU_ID,
            label: Some("File".to_owned()),
            enabled: true,
            kind: MenuKind::Submenu,
            icon: None,
            children: vec![MenuNode {
                id: QUIT_ITEM_ID,
                label: Some("Quit".to_owned()),
                enabled: true,
                kind: MenuKind::Standard,
                icon: None,
                children: Vec::new(),
            }],
        }]
    }

    fn qt_menu_bar_location() -> DbusMenuLocation {
        DbusMenuLocation {
            bus_name: OwnedBusName::try_from(":1.40").expect("bus name"),
            menu_path: OwnedObjectPath::try_from("/MenuBar/1").expect("object path"),
        }
    }

    fn group_showing(activation_route: MenuActivationRoute) -> GlobalMenuGroup {
        let mut group = GlobalMenuGroup::default();
        let _ = group.update(GlobalMenuMessage::MenuBarChanged(FocusedMenuBar::Shown(
            ShownMenuBar {
                top_menus: file_menu_with_quit(),
                activation_route,
                menu_bar_match: MenuBarMatch::ExactWindow,
            },
        )));
        group
    }

    fn gtk_quit_route() -> MenuActivationRoute {
        MenuActivationRoute::GtkActions {
            action_groups: GtkActionGroups {
                bus_name: OwnedBusName::try_from("org.example.Editor").expect("bus name"),
                application_actions: ApplicationActionPath::Exported(
                    OwnedObjectPath::try_from("/org/example/Editor").expect("object path"),
                ),
                window_actions: WindowActionPath::Unresolved,
            },
            activations: HashMap::from([(
                QUIT_ITEM_ID,
                GtkMenuActivation {
                    scope: GtkActionScope::Application,
                    action_name: "quit".to_owned(),
                    action_target: GtkActionTarget::Untargeted,
                },
            )]),
        }
    }

    #[test]
    fn entering_a_qt_top_menu_asks_it_to_prepare_its_items() {
        let mut group = group_showing(MenuActivationRoute::DbusMenu(qt_menu_bar_location()));
        let action = group.update(GlobalMenuMessage::TopMenuEntered(FILE_MENU_ID));
        assert!(matches!(
            action,
            Action::Prepare(GlobalMenuRequest::OpenDbusMenuSubmenu {
                item_id: FILE_MENU_ID,
                ..
            })
        ));
    }

    #[test]
    fn pressing_a_qt_item_clicks_it_by_id() {
        let mut group = group_showing(MenuActivationRoute::DbusMenu(qt_menu_bar_location()));
        let action = group.update(GlobalMenuMessage::ItemPressed(QUIT_ITEM_ID));
        assert!(matches!(
            action,
            Action::Activate(GlobalMenuRequest::ClickDbusMenuItem {
                item_id: QUIT_ITEM_ID,
                ..
            })
        ));
    }

    #[test]
    fn pressing_a_gtk_item_activates_its_action() {
        let mut group = group_showing(gtk_quit_route());
        let Action::Activate(GlobalMenuRequest::ActivateGtkAction { activation, .. }) =
            group.update(GlobalMenuMessage::ItemPressed(QUIT_ITEM_ID))
        else {
            panic!("the quit item activates a gtk action");
        };
        assert_eq!(activation.action_name, "quit");
    }

    #[test]
    fn entering_a_gtk_top_menu_needs_no_preparation() {
        let mut group = group_showing(gtk_quit_route());
        assert!(matches!(
            group.update(GlobalMenuMessage::TopMenuEntered(FILE_MENU_ID)),
            Action::None
        ));
    }

    #[test]
    fn focus_change_drops_the_previous_menu_bar() {
        let mut group = group_showing(gtk_quit_route());
        group.observe_focused_window(FocusedWindowIdentity::Window {
            window_id: 9,
            owner: FocusedWindowOwner::ProcessOnly { process_id: 9 },
        });
        assert!(
            group
                .menu_panel(
                    GlobalMenuTarget::TopMenu(FILE_MENU_ID),
                    &Config::default(),
                    Radius::from(0.0)
                )
                .is_none()
        );
    }

    #[test]
    fn expanded_group_lays_out_each_top_menu_label() {
        let config = Config::default();
        let mut group = group_showing(gtk_quit_route());
        let _ = group.update(GlobalMenuMessage::Expansion(
            WidgetGroupExpansionMessage::Expand,
        ));
        let panel_view = group
            .group_content(&config)
            .view_events(&config, Position::Top, Clock::real())
            .expect("a group with top menus is drawn");
        let mut simulator = Simulator::with_size(
            iced::Settings::default(),
            iced::Size::new(800.0, 40.0),
            panel_view,
        );
        assert!(simulator.find("File").is_ok());
    }

    #[test]
    fn group_without_menu_bar_is_not_drawn() {
        let config = Config::default();
        let group = GlobalMenuGroup::default();
        assert!(
            group
                .group_content(&config)
                .view_events(&config, Position::Top, Clock::real())
                .is_none()
        );
    }
}
