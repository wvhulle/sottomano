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
        application_menu_nodes::MenuNode,
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
};
use sottomano_toml_config::Config;

use self::top_menu_glyphs::top_menu_glyph;
use crate::{
    background_work::logged_failure_task,
    cascading_menus::menu_node_entries::menu_node_entries,
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
        toggle_control::{IconToggle, SwitchPosition, SwitchPresses},
    },
    group_menus::group_menu_state::MenuTopicPresence,
    hover_cards::hover_card_body::{Section, SectionWidget},
    indicator_groups::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
        sqlite_singleton_flag::{self, SingletonFlag, SingletonFlagState},
    },
    widgets::{
        icon_glyph::IconGlyph,
        icon_handle::PixmapHandles,
        phosphor::{Phosphor, Weight},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowMenuBarUse {
    Enabled,
    Disabled,
}

impl WindowMenuBarUse {
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            WindowMenuBarUse::Enabled => WindowMenuBarUse::Disabled,
            WindowMenuBarUse::Disabled => WindowMenuBarUse::Enabled,
        }
    }
}

impl SingletonFlagState for WindowMenuBarUse {
    const STORED_FLAG: SingletonFlag = SingletonFlag::WindowMenuBarUse;
    const STORED_AS_SET: Self = WindowMenuBarUse::Enabled;
    const STORED_AS_CLEARED: Self = WindowMenuBarUse::Disabled;
}

impl From<WindowMenuBarUse> for SwitchPosition {
    fn from(window_menu_bar_use: WindowMenuBarUse) -> Self {
        match window_menu_bar_use {
            WindowMenuBarUse::Enabled => SwitchPosition::On,
            WindowMenuBarUse::Disabled => SwitchPosition::Off,
        }
    }
}

fn window_menu_bar_use_toggle(window_menu_bar_use: WindowMenuBarUse) -> IconToggle {
    IconToggle::new(
        "Global application menu",
        Phosphor::AppWindow(Weight::Regular),
        window_menu_bar_use,
        SwitchPresses {
            turning_on: "show the focused window's menus in a panel menu; Qt apps started \
                         afterwards hide their own menu bar",
            turning_off: "leave menus inside their windows",
        },
    )
}

struct MenuLookupFailureIndicator<'failure> {
    lookup_failure: &'failure X11MenuLookupError,
}

impl<'failure> MenuLookupFailureIndicator<'failure> {
    fn indicator(self) -> GroupIndicator<'failure> {
        GroupIndicator::glyph_only(
            "Menu lookup",
            Phosphor::Warning(Weight::Regular).into(),
            IndicatorState::Failed,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled("failed".to_owned()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Error",
            SectionWidget::paragraph(self.lookup_failure.to_string()),
        )]
    }
}

pub struct WindowMenuBar {
    pub window_menu_bar_use: WindowMenuBarUse,
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
            window_menu_bar_use: WindowMenuBarUse::Disabled,
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
    RegistrationsChanged(Vec<RegisteredMenuBar>),
    MenuBarChanged(FocusedMenuBar),
    SubmenusPrepared,
}

#[derive(Debug, Clone)]
pub enum WindowMenuBarInteraction {
    ToggleUse,
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
    Activate(WindowMenuBarRequest),
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
        Task::perform(
            sqlite_singleton_flag::load(WindowMenuBarUse::Disabled),
            WindowMenuBarMessage::UseLoaded,
        )
    }

    #[must_use]
    pub fn shows_top_menus(&self) -> bool {
        self.window_menu_bar_use == WindowMenuBarUse::Enabled
            && matches!(self.focused_menu_bar, FocusedMenuBar::Shown(_))
    }

    pub fn collapse(&mut self) {
        self.switch_expansion.collapse();
    }

    pub fn close_menu(&mut self) {
        self.menu_state.close();
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
            WindowMenuBarInteraction::Menu(menu_action) => {
                self.menu_state.perform(menu_action);
                Action::None
            }
            WindowMenuBarInteraction::SwitchExpansion(expansion_message) => {
                self.switch_expansion.update(expansion_message);
                Action::None
            }
            WindowMenuBarInteraction::ItemPressed(item_id) => {
                self.menu_state.close();
                self.item_activation(item_id)
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

    fn item_activation(&self, item_id: i32) -> Action {
        let FocusedMenuBar::Shown(shown_menu_bar) = &self.focused_menu_bar else {
            return Action::None;
        };
        match &shown_menu_bar.activation_route {
            MenuActivationRoute::DbusMenu(location) => {
                Action::Activate(WindowMenuBarRequest::ClickDbusMenuItem {
                    location: location.clone(),
                    item_id,
                })
            }
            MenuActivationRoute::GtkActions {
                action_groups,
                activations,
            } => match activations.get(&item_id) {
                Some(activation) => Action::Activate(WindowMenuBarRequest::ActivateGtkAction {
                    action_groups: action_groups.clone(),
                    activation: activation.clone(),
                }),
                None => Action::None,
            },
        }
    }

    pub fn switch_content(&self) -> IndicatorGroupContent<'_> {
        let mut rows = vec![GroupIndicator::toggle(
            window_menu_bar_use_toggle(self.window_menu_bar_use),
            WindowMenuBarInteraction::ToggleUse,
        )];
        if let Some(lookup_failure) = self.lookup_failure() {
            rows.push(MenuLookupFailureIndicator { lookup_failure }.indicator());
        }
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::WindowMenuBarSwitch,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::WindowMenuBarSwitch.title(),
                Phosphor::CursorClick(Weight::Regular).into(),
                match self.lookup_failure() {
                    Some(_) => IndicatorState::Failed,
                    None => IndicatorState::Idle,
                },
            ),
            &self.switch_expansion,
            WindowMenuBarInteraction::SwitchExpansion,
            rows,
        )
    }

    #[must_use]
    pub fn menu_entries(
        &self,
        config: &Config,
    ) -> Vec<MenuEntry<'static, i32, WindowMenuBarInteraction>> {
        if self.window_menu_bar_use == WindowMenuBarUse::Disabled {
            return Vec::new();
        }
        let icon_size_px = config.theme.font.icon_size_px;
        self.top_menus()
            .iter()
            .filter_map(|top_menu| {
                let top_menu_label = top_menu.label.clone()?;
                let header = MenuRowContent::new(top_menu_label.clone()).icon(
                    IconGlyph::from(top_menu_glyph(&top_menu_label)).view(icon_size_px, text_color),
                );
                let child_entries = menu_node_entries(
                    &top_menu.children,
                    config,
                    &self.pixmap_handles,
                    &WindowMenuBarInteraction::ItemPressed,
                );
                Some(MenuSubmenu::new(top_menu.id, header, child_entries).into())
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use sottomano_desktop_sources::desktop_windows_apps::{
        application_menu_nodes::MenuKind,
        focused_window_menu_bar::{FocusedWindowOwner, MenuBarMatch, ShownMenuBar},
        gtk_menus_menu_bar::{
            ApplicationActionPath, GtkActionScope, GtkActionTarget, WindowActionPath,
        },
    };
    use zbus::{names::OwnedBusName, zvariant::OwnedObjectPath};

    use super::*;

    const FILE_MENU_ID: i32 = 1;
    const QUIT_ITEM_ID: i32 = 2;
    const RECENT_MENU_ID: i32 = 3;
    const RECENT_FILE_ID: i32 = 4;
    const SEPARATOR_ID: i32 = 5;
    const PRINT_ITEM_ID: i32 = 6;

    fn menu_node(id: i32, label: &str, kind: MenuKind, children: Vec<MenuNode>) -> MenuNode {
        MenuNode {
            id,
            label: Some(label.to_owned()),
            enabled: true,
            kind,
            icon: None,
            children,
        }
    }

    fn file_menu() -> Vec<MenuNode> {
        vec![menu_node(
            FILE_MENU_ID,
            "File",
            MenuKind::Submenu,
            vec![
                menu_node(
                    RECENT_MENU_ID,
                    "Recent",
                    MenuKind::Submenu,
                    vec![menu_node(
                        RECENT_FILE_ID,
                        "notes.txt",
                        MenuKind::Standard,
                        Vec::new(),
                    )],
                ),
                MenuNode {
                    label: None,
                    ..menu_node(SEPARATOR_ID, "", MenuKind::Separator, Vec::new())
                },
                MenuNode {
                    enabled: false,
                    ..menu_node(PRINT_ITEM_ID, "Print", MenuKind::Standard, Vec::new())
                },
                menu_node(QUIT_ITEM_ID, "Quit", MenuKind::Standard, Vec::new()),
            ],
        )]
    }

    fn qt_menu_bar_location() -> DbusMenuLocation {
        DbusMenuLocation {
            bus_name: OwnedBusName::try_from(":1.40").expect("bus name"),
            menu_path: OwnedObjectPath::try_from("/MenuBar/1").expect("object path"),
        }
    }

    fn enabled_group_showing(activation_route: MenuActivationRoute) -> WindowMenuBar {
        let mut group = WindowMenuBar::default();
        let _ = group.update(WindowMenuBarMessage::UseLoaded(WindowMenuBarUse::Enabled));
        group.observe_focused_window(FocusedWindowIdentity::Window {
            window_id: 3,
            owner: FocusedWindowOwner::ProcessAndApp {
                process_id: 40,
                app_id: "org.example.Editor".to_owned(),
            },
        });
        let _ = group.update(WindowMenuBarMessage::MenuBarChanged(FocusedMenuBar::Shown(
            ShownMenuBar {
                top_menus: file_menu(),
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
    fn each_labelled_top_menu_becomes_a_submenu() {
        let group = enabled_group_showing(MenuActivationRoute::DbusMenu(qt_menu_bar_location()));
        let top_menu_entries = group.menu_entries(&Config::default());
        assert_eq!(top_menu_entries.len(), 1);
        assert!(matches!(top_menu_entries[0], MenuEntry::Submenu(_)));
        assert!(group.shows_top_menus());
    }

    #[test]
    fn submenu_children_keep_their_nesting_and_separators() {
        let group = enabled_group_showing(MenuActivationRoute::DbusMenu(qt_menu_bar_location()));
        let file_children = menu_node_entries(
            &file_menu()[0].children,
            &Config::default(),
            &group.pixmap_handles,
            &WindowMenuBarInteraction::ItemPressed,
        );
        assert!(matches!(
            file_children.as_slice(),
            [
                MenuEntry::Submenu(_),
                MenuEntry::Divider,
                MenuEntry::Item(_),
                MenuEntry::Item(_)
            ]
        ));
    }

    #[test]
    fn switched_off_group_shows_nothing() {
        let mut group =
            enabled_group_showing(MenuActivationRoute::DbusMenu(qt_menu_bar_location()));
        let _ = group.interact(WindowMenuBarInteraction::ToggleUse);
        assert_eq!(group.window_menu_bar_use, WindowMenuBarUse::Disabled);
        assert!(group.menu_entries(&Config::default()).is_empty());
        assert!(!group.shows_top_menus());
    }

    #[test]
    fn pressing_an_entry_closes_the_open_submenus() {
        let mut group =
            enabled_group_showing(MenuActivationRoute::DbusMenu(qt_menu_bar_location()));
        let _ = group.interact(WindowMenuBarInteraction::Menu(NestedMenuAction::Opened(
            vec![FILE_MENU_ID, RECENT_MENU_ID],
        )));
        let _ = group.interact(WindowMenuBarInteraction::ItemPressed(RECENT_FILE_ID));
        assert!(group.menu_state.open_submenus.is_empty());
    }

    #[test]
    fn group_starts_switched_off() {
        assert_eq!(
            WindowMenuBar::default().window_menu_bar_use,
            WindowMenuBarUse::Disabled
        );
    }

    #[test]
    fn pressing_a_qt_entry_clicks_it_by_id() {
        let mut group =
            enabled_group_showing(MenuActivationRoute::DbusMenu(qt_menu_bar_location()));
        assert!(matches!(
            group.interact(WindowMenuBarInteraction::ItemPressed(QUIT_ITEM_ID)),
            Action::Activate(WindowMenuBarRequest::ClickDbusMenuItem {
                item_id: QUIT_ITEM_ID,
                ..
            })
        ));
    }

    #[test]
    fn pressing_a_gtk_entry_activates_its_action() {
        let mut group = enabled_group_showing(gtk_quit_route());
        let Action::Activate(WindowMenuBarRequest::ActivateGtkAction { activation, .. }) =
            group.interact(WindowMenuBarInteraction::ItemPressed(QUIT_ITEM_ID))
        else {
            panic!("the quit entry activates a gtk action");
        };
        assert_eq!(activation.action_name, "quit");
    }

    #[test]
    fn menu_bar_survives_the_panel_taking_keyboard_focus() {
        let mut group = enabled_group_showing(gtk_quit_route());
        group.observe_focused_window(FocusedWindowIdentity::NoFocusedWindow);
        assert!(group.shows_top_menus());
        assert_eq!(group.menu_entries(&Config::default()).len(), 1);
    }

    #[test]
    fn focus_change_drops_the_previous_menu_bar() {
        let mut group = enabled_group_showing(gtk_quit_route());
        group.observe_focused_window(FocusedWindowIdentity::Window {
            window_id: 9,
            owner: FocusedWindowOwner::ProcessOnly { process_id: 9 },
        });
        assert!(group.menu_entries(&Config::default()).is_empty());
    }
}
