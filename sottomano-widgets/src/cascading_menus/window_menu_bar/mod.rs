pub mod inline_top_menus;
pub mod menu_bar_switch;
pub mod top_menu_glyphs;

use corredo::{
    popovers::{MenuEntry, MenuRowContent, MenuSubmenu, NestedMenuAction, NestedMenuState},
    theming::text_color,
};
use iced::{
    Subscription, Task,
    futures::{Stream, stream},
};
use sottomano_desktop_sinks::{
    desktop_windows_apps::{
        dbusmenu_item_events::{click_dbusmenu_item, prepare_dbusmenu_submenus},
        gtk_action_activation::activate_gtk_action,
    },
    sink_write_errors::BusWriteError,
};
use sottomano_desktop_sources::{
    desktop_windows_apps::{
        application_menu_nodes::{MenuLabel, MenuNode},
        appmenu_registrar_server::{self, RegisteredMenuBar},
        dbusmenu_menu_bar_layout::DbusMenuLocation,
        focused_window_menu_bar::{
            FocusedMenuBar, FocusedWindowIdentity, MenuActivationRoute, MenuBarQuery,
            focused_menu_bar_updates,
        },
        gtk_menus_menu_bar::{GtkActionGroups, GtkMenuActivation},
        x11_window_menu_properties::X11MenuLookupError,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    theme_icons::icon_requests::IconRequest,
};
use sottomano_toml_config::Config;

use self::{
    menu_bar_switch::{WindowMenuBarLayout, WindowMenuBarUse},
    top_menu_glyphs::top_menu_glyph,
};
use crate::{
    background_work::logged_failure_task,
    cascading_menus::menu_node_entries::{
        PressDismissal, menu_icon_side_px, menu_node_icon_requests, press_dismissal,
    },
    glyph_icons::icon_glyph::IconGlyph,
    group_indicators::{
        indicator_group_content::{IndicatorGroupExpansion, IndicatorGroupExpansionMessage},
        sqlite_singleton_flag,
    },
    group_menus::group_menu_state::MenuTopicPresence,
    icon_catalog::{drawable_icon_catalog::IconCatalog, pixmap_handles::PixmapHandles},
};

pub struct WindowMenuBar {
    pub window_menu_bar_use: WindowMenuBarUse,
    pub window_menu_bar_layout: WindowMenuBarLayout,
    registered_menu_bars: Vec<RegisteredMenuBar>,
    focused_window: FocusedWindowIdentity,
    focused_menu_bar: FocusedMenuBar,
    pixmap_handles: PixmapHandles,
    pub menu_state: NestedMenuState<i32>,
    switch_expansion: IndicatorGroupExpansion,
}

impl Default for WindowMenuBar {
    fn default() -> Self {
        Self {
            window_menu_bar_use: WindowMenuBarUse::default(),
            window_menu_bar_layout: WindowMenuBarLayout::default(),
            registered_menu_bars: Vec::new(),
            focused_window: FocusedWindowIdentity::NoFocusedWindow,
            focused_menu_bar: FocusedMenuBar::NoFocusedWindow,
            pixmap_handles: PixmapHandles::default(),
            menu_state: NestedMenuState::default(),
            switch_expansion: IndicatorGroupExpansion::default(),
        }
    }
}

#[derive(Debug)]
pub enum WindowMenuBarMessage {
    UseLoaded(WindowMenuBarUse),
    LayoutLoaded(WindowMenuBarLayout),
    RegistrationsChanged(Vec<RegisteredMenuBar>),
    MenuBarChanged(FocusedMenuBar),
    SubmenusPrepared,
}

#[derive(Debug, Clone)]
pub enum WindowMenuBarInteraction {
    ToggleUse,
    ToggleLayout,
    ItemPressed(i32),
    Menu(NestedMenuAction<i32>),
    SwitchExpansion(IndicatorGroupExpansionMessage),
}

#[derive(Debug, Clone)]
pub enum WindowMenuBarRequest {
    ClickDbusMenuItem {
        location: DbusMenuLocation,
        item_id: i32,
    },
    ActivateGtkAction {
        action_groups: GtkActionGroups,
        activation: GtkMenuActivation,
    },
}

impl WindowMenuBarRequest {
    pub async fn send(self, session_bus: zbus::Connection) -> Result<(), BusWriteError> {
        match self {
            WindowMenuBarRequest::ClickDbusMenuItem { location, item_id } => {
                click_dbusmenu_item(session_bus, location, item_id).await
            }
            WindowMenuBarRequest::ActivateGtkAction {
                action_groups,
                activation,
            } => activate_gtk_action(session_bus, action_groups, activation).await,
        }
    }
}

pub enum Action {
    None,
    Run(Task<WindowMenuBarMessage>),
    Activate {
        request: WindowMenuBarRequest,
        dismissal: PressDismissal,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SubmenuPreparation {
    buses: DesktopBuses,
    location: DbusMenuLocation,
    top_menu_ids: Vec<i32>,
}

fn prepared_submenus(
    preparation: &SubmenuPreparation,
) -> impl Stream<Item = WindowMenuBarMessage> + Send + use<> {
    let preparation = preparation.clone();
    stream::once(async move {
        prepare_dbusmenu_submenus(
            preparation.buses.session.clone(),
            preparation.location,
            preparation.top_menu_ids,
        )
        .await;
        WindowMenuBarMessage::SubmenusPrepared
    })
}

impl WindowMenuBar {
    pub fn load() -> Task<WindowMenuBarMessage> {
        Task::batch([
            Task::perform(
                sqlite_singleton_flag::load(WindowMenuBarUse::default()),
                WindowMenuBarMessage::UseLoaded,
            ),
            Task::perform(
                sqlite_singleton_flag::load(WindowMenuBarLayout::default()),
                WindowMenuBarMessage::LayoutLoaded,
            ),
        ])
    }

    #[must_use]
    pub fn shows_top_menus(&self) -> bool {
        self.window_menu_bar_use == WindowMenuBarUse::Enabled
            && matches!(self.focused_menu_bar, FocusedMenuBar::Shown(_))
    }

    pub fn collapse(&mut self) {
        self.switch_expansion.collapse();
    }

    pub fn observe_focused_window(&mut self, focused_window: FocusedWindowIdentity) {
        if focused_window == self.focused_window
            || focused_window == FocusedWindowIdentity::NoFocusedWindow
        {
            return;
        }
        self.focused_window = focused_window;
        self.focused_menu_bar = FocusedMenuBar::NotExported;
        self.menu_state.close();
    }

    fn top_menus(&self) -> &[MenuNode] {
        match &self.focused_menu_bar {
            FocusedMenuBar::Shown(shown_menu_bar) => &shown_menu_bar.top_menus,
            FocusedMenuBar::NotExported
            | FocusedMenuBar::NoFocusedWindow
            | FocusedMenuBar::LookupFailed(_) => &[],
        }
    }

    fn lookup_failure(&self) -> Option<&X11MenuLookupError> {
        match (&self.window_menu_bar_use, &self.focused_menu_bar) {
            (WindowMenuBarUse::Enabled, FocusedMenuBar::LookupFailed(lookup_failure)) => {
                Some(lookup_failure)
            }
            (WindowMenuBarUse::Enabled | WindowMenuBarUse::Disabled, _) => None,
        }
    }

    pub fn subscription(
        &self,
        buses: &DesktopBuses,
        window_menu_bar_presence: MenuTopicPresence,
    ) -> Subscription<WindowMenuBarMessage> {
        if self.window_menu_bar_use == WindowMenuBarUse::Disabled {
            return Subscription::none();
        }
        let registrar = Subscription::run(appmenu_registrar_server::registrations)
            .map(WindowMenuBarMessage::RegistrationsChanged);
        let menu_bar_query = MenuBarQuery {
            buses: buses.clone(),
            focused_window: self.focused_window.clone(),
            registered_menu_bars: self.registered_menu_bars.clone(),
        };
        let submenu_preparation = match &self.focused_menu_bar {
            FocusedMenuBar::Shown(shown_menu_bar) => match &shown_menu_bar.activation_route {
                MenuActivationRoute::DbusMenu(location) => match window_menu_bar_presence {
                    MenuTopicPresence::RecentlyOpen => Subscription::run_with(
                        SubmenuPreparation {
                            buses: buses.clone(),
                            location: location.clone(),
                            top_menu_ids: shown_menu_bar
                                .top_menus
                                .iter()
                                .map(|top_menu| top_menu.id)
                                .collect(),
                        },
                        prepared_submenus,
                    ),
                    MenuTopicPresence::Closed => Subscription::none(),
                },
                MenuActivationRoute::GtkActions { .. } => Subscription::none(),
            },
            FocusedMenuBar::NotExported
            | FocusedMenuBar::NoFocusedWindow
            | FocusedMenuBar::LookupFailed(_) => Subscription::none(),
        };
        Subscription::batch([
            registrar,
            Subscription::run_with(menu_bar_query, focused_menu_bar_updates)
                .map(WindowMenuBarMessage::MenuBarChanged),
            submenu_preparation,
        ])
    }

    pub fn update(&mut self, message: WindowMenuBarMessage) -> Action {
        match message {
            WindowMenuBarMessage::UseLoaded(window_menu_bar_use) => {
                self.window_menu_bar_use = window_menu_bar_use;
                Action::None
            }
            WindowMenuBarMessage::LayoutLoaded(window_menu_bar_layout) => {
                self.window_menu_bar_layout = window_menu_bar_layout;
                Action::None
            }
            WindowMenuBarMessage::RegistrationsChanged(registered_menu_bars) => {
                self.registered_menu_bars = registered_menu_bars;
                Action::None
            }
            WindowMenuBarMessage::MenuBarChanged(focused_menu_bar) => {
                self.adopt_menu_bar(focused_menu_bar);
                Action::None
            }
            WindowMenuBarMessage::SubmenusPrepared => Action::None,
        }
    }

    pub fn interact(&mut self, interaction: WindowMenuBarInteraction) -> Action {
        match interaction {
            WindowMenuBarInteraction::ToggleUse => {
                self.window_menu_bar_use = self.window_menu_bar_use.toggled();
                if self.window_menu_bar_use == WindowMenuBarUse::Disabled {
                    self.registered_menu_bars.clear();
                    self.focused_menu_bar = FocusedMenuBar::NotExported;
                    self.menu_state.close();
                }
                Action::Run(logged_failure_task(
                    "singleton flag: save".to_owned(),
                    sqlite_singleton_flag::save(self.window_menu_bar_use),
                ))
            }
            WindowMenuBarInteraction::ToggleLayout => {
                self.window_menu_bar_layout = self.window_menu_bar_layout.toggled();
                self.menu_state.close();
                Action::Run(logged_failure_task(
                    "singleton flag: save".to_owned(),
                    sqlite_singleton_flag::save(self.window_menu_bar_layout),
                ))
            }
            WindowMenuBarInteraction::Menu(menu_action) => {
                self.menu_state.perform(menu_action);
                Action::None
            }
            WindowMenuBarInteraction::SwitchExpansion(expansion_message) => {
                self.switch_expansion.update(expansion_message);
                Action::None
            }
            WindowMenuBarInteraction::ItemPressed(item_id) => {
                let dismissal = press_dismissal(self.top_menus(), item_id);
                if dismissal == PressDismissal::ClosesMenu {
                    self.menu_state.close();
                }
                self.item_activation(item_id, dismissal)
            }
        }
    }

    fn adopt_menu_bar(&mut self, focused_menu_bar: FocusedMenuBar) {
        if focused_menu_bar == self.focused_menu_bar {
            return;
        }
        if let FocusedMenuBar::Shown(shown_menu_bar) = &focused_menu_bar {
            self.pixmap_handles = self
                .pixmap_handles
                .refreshed(shown_menu_bar.top_menus.iter().flat_map(MenuNode::pixmaps));
        }
        self.focused_menu_bar = focused_menu_bar;
    }

    fn item_activation(&self, item_id: i32, dismissal: PressDismissal) -> Action {
        let FocusedMenuBar::Shown(shown_menu_bar) = &self.focused_menu_bar else {
            return Action::None;
        };
        match &shown_menu_bar.activation_route {
            MenuActivationRoute::DbusMenu(location) => Action::Activate {
                request: WindowMenuBarRequest::ClickDbusMenuItem {
                    location: location.clone(),
                    item_id,
                },
                dismissal,
            },
            MenuActivationRoute::GtkActions {
                action_groups,
                activations,
            } => match activations.get(&item_id) {
                Some(activation) => Action::Activate {
                    request: WindowMenuBarRequest::ActivateGtkAction {
                        action_groups: action_groups.clone(),
                        activation: activation.clone(),
                    },
                    dismissal,
                },
                None => Action::None,
            },
        }
    }

    #[must_use]
    pub fn icon_requests(&self, config: &Config) -> Vec<IconRequest> {
        menu_node_icon_requests(self.top_menus(), menu_icon_side_px(config))
    }

    #[must_use]
    pub fn menu_entries(
        &self,
        config: &Config,
        icon_catalog: &IconCatalog,
    ) -> Vec<MenuEntry<'static, i32, WindowMenuBarInteraction>> {
        if self.window_menu_bar_use == WindowMenuBarUse::Disabled {
            return Vec::new();
        }
        let icon_size_px = config.theme.font.icon_size_px;
        self.top_menus()
            .iter()
            .filter_map(|top_menu| {
                let MenuLabel::Labelled(top_menu_label) = top_menu.label.clone() else {
                    return None;
                };
                let header = MenuRowContent::new(top_menu_label.clone()).icon(
                    IconGlyph::from(top_menu_glyph(&top_menu_label)).view(icon_size_px, text_color),
                );
                let child_entries = self.top_menu_children(top_menu, config, icon_catalog);
                Some(MenuSubmenu::new(top_menu.id, header, child_entries).into())
            })
            .collect()
    }
}

#[cfg(test)]
mod window_menu_bar_tests;
