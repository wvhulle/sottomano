pub mod top_menu_glyphs;

use chrono::Locale;
use iced::{
    Subscription, Task,
    futures::{Stream, stream},
};
use sottomano_desktop_sinks::desktop_windows_apps::{
    dbusmenu_item_events::{click_dbusmenu_item, prepare_dbusmenu_submenus},
    gtk_action_activation::activate_gtk_action,
};
use sottomano_desktop_sources::{
    desktop_windows_apps::{
        application_menu_nodes::{MenuKind, MenuNode},
        appmenu_registrar_server::{self, RegisteredMenuBar},
        dbusmenu_menu_bar_layout::DbusMenuLocation,
        focused_window_menu_bar::{
            FocusedMenuBar, FocusedWindowIdentity, FocusedWindowOwner, MenuActivationRoute,
            MenuBarMatch, MenuBarQuery, focused_menu_bar_updates,
        },
        gtk_menus_menu_bar::{GtkActionGroups, GtkMenuActivation},
        x11_window_menu_properties::X11MenuLookupError,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use self::top_menu_glyphs::top_menu_glyph;
use crate::{
    background_work::logged_failure_task,
    clock::Clock,
    group_items::{
        group_item::{ComposedGroupItemFace, GroupItem, GroupItemIcon, GroupItemValue},
        group_item_reading::GroupItemReading,
        reading_display_state::ReadingDisplayState,
        toggle_control::{IconToggle, SwitchPosition, SwitchPresses},
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::{
        HoverTarget,
        hover_card_body::{
            HoverCardBody, Section,
            hover_card_source_delivery::{DeliveryTiming, HoverCard},
        },
    },
    widget_groups::{
        sqlite_singleton_flag::{self, SingletonFlag, SingletonFlagState},
        widget_group_content::{
            KeyedExpansions, WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
            WidgetGroupFace,
        },
        widget_group_identity::{
            WidgetGroupFamily, WidgetGroupIdentity, WidgetGroupInstance, WidgetGroupTitle,
        },
    },
    widgets::{
        icon_glyph::IconGlyph,
        icon_handle::PixmapHandles,
        phosphor::{Phosphor, Weight},
    },
};

const LABEL_PATH_SEPARATOR: &str = " › ";
const MAXIMUM_SUBMENU_DEPTH: usize = 3;
const UNIDENTIFIED_APPLICATION: &str = "unidentified";
const RUN_ENTRY_HINT: &str = "run";
const MOST_RECENT_OF_SEVERAL_NOTE: &str =
    "This application exports several menu bars; the most recently created one is shown.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalMenuUse {
    Enabled,
    Disabled,
}

impl GlobalMenuUse {
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            GlobalMenuUse::Enabled => GlobalMenuUse::Disabled,
            GlobalMenuUse::Disabled => GlobalMenuUse::Enabled,
        }
    }
}

impl SingletonFlagState for GlobalMenuUse {
    const STORED_FLAG: SingletonFlag = SingletonFlag::GlobalApplicationMenu;
    const STORED_AS_SET: Self = GlobalMenuUse::Enabled;
    const STORED_AS_CLEARED: Self = GlobalMenuUse::Disabled;
}

impl From<GlobalMenuUse> for SwitchPosition {
    fn from(global_menu_use: GlobalMenuUse) -> Self {
        match global_menu_use {
            GlobalMenuUse::Enabled => SwitchPosition::On,
            GlobalMenuUse::Disabled => SwitchPosition::Off,
        }
    }
}

fn global_menu_use_toggle(global_menu_use: GlobalMenuUse) -> IconToggle {
    IconToggle::new(
        "Global application menu",
        Phosphor::AppWindow(Weight::Regular),
        global_menu_use,
        SwitchPresses {
            turning_on: "show the focused window's menus in a panel menu; Qt apps started \
                         afterwards hide their own menu bar",
            turning_off: "leave menus inside their windows",
        },
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlobalMenuTarget {
    Use,
    LookupFailure,
    Entry { app_id: String, label_path: String },
}

impl From<GlobalMenuTarget> for HoverTarget {
    fn from(target: GlobalMenuTarget) -> Self {
        HoverTarget::GlobalMenu(target)
    }
}

struct MenuLookupFailureReading<'failure> {
    lookup_failure: &'failure X11MenuLookupError,
}

impl GroupItemReading for MenuLookupFailureReading<'_> {
    fn name(&self) -> &'static str {
        "Menu lookup"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Warning(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some("failed".to_owned())
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::Failed
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::from(Section::error(self.lookup_failure))
    }
}

struct FlattenedEntry<'menu> {
    node: &'menu MenuNode,
    label_path: String,
}

fn flattened_entries(top_menu: &MenuNode) -> Vec<FlattenedEntry<'_>> {
    let mut entries = Vec::new();
    collect_entries(&top_menu.children, "", 0, &mut entries);
    entries
}

fn collect_entries<'menu>(
    nodes: &'menu [MenuNode],
    parent_path: &str,
    depth: usize,
    entries: &mut Vec<FlattenedEntry<'menu>>,
) {
    for node in nodes {
        let Some(label) = node.label.as_deref() else {
            continue;
        };
        let label_path = if parent_path.is_empty() {
            label.to_owned()
        } else {
            format!("{parent_path}{LABEL_PATH_SEPARATOR}{label}")
        };
        match node.kind {
            MenuKind::Separator => {}
            MenuKind::Submenu => {
                if depth + 1 < MAXIMUM_SUBMENU_DEPTH {
                    collect_entries(&node.children, &label_path, depth + 1, entries);
                }
            }
            MenuKind::Standard | MenuKind::Toggle(_) => {
                entries.push(FlattenedEntry { node, label_path });
            }
        }
    }
}

pub struct GlobalMenuGroup {
    global_menu_use: GlobalMenuUse,
    registered_menu_bars: Vec<RegisteredMenuBar>,
    focused_window: FocusedWindowIdentity,
    focused_menu_bar: FocusedMenuBar,
    pixmap_handles: PixmapHandles,
    expansions: KeyedExpansions<String>,
    switch_expansion: WidgetGroupExpansion,
}

impl Default for GlobalMenuGroup {
    fn default() -> Self {
        Self {
            global_menu_use: GlobalMenuUse::Disabled,
            registered_menu_bars: Vec::new(),
            focused_window: FocusedWindowIdentity::NoFocusedWindow,
            focused_menu_bar: FocusedMenuBar::NoFocusedWindow,
            pixmap_handles: PixmapHandles::default(),
            expansions: KeyedExpansions::default(),
            switch_expansion: WidgetGroupExpansion::default(),
        }
    }
}

#[derive(Debug)]
pub enum GlobalMenuMessage {
    UseLoaded(GlobalMenuUse),
    RegistrationsChanged(Vec<RegisteredMenuBar>),
    MenuBarChanged(FocusedMenuBar),
    SubmenusPrepared,
    Interaction(GlobalMenuInteraction),
}

#[derive(Debug, Clone)]
pub enum GlobalMenuInteraction {
    ToggleUse,
    ItemPressed(i32),
    Expansion {
        top_menu_label: String,
        expansion_message: WidgetGroupExpansionMessage,
    },
    SwitchExpansion(WidgetGroupExpansionMessage),
}

#[derive(Debug, Clone)]
pub enum GlobalMenuRequest {
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
    Run(Task<GlobalMenuMessage>),
    Activate(GlobalMenuRequest),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SubmenuPreparation {
    buses: DesktopBuses,
    location: DbusMenuLocation,
    top_menu_ids: Vec<i32>,
}

fn prepared_submenus(
    preparation: &SubmenuPreparation,
) -> impl Stream<Item = GlobalMenuMessage> + Send + use<> {
    let preparation = preparation.clone();
    stream::once(async move {
        prepare_dbusmenu_submenus(
            preparation.buses.session.clone(),
            preparation.location,
            preparation.top_menu_ids,
        )
        .await;
        GlobalMenuMessage::SubmenusPrepared
    })
}

impl GlobalMenuGroup {
    pub fn load() -> Task<GlobalMenuMessage> {
        Task::perform(
            sqlite_singleton_flag::load(GlobalMenuUse::Disabled),
            GlobalMenuMessage::UseLoaded,
        )
    }

    #[must_use]
    pub fn global_menu_use(&self) -> GlobalMenuUse {
        self.global_menu_use
    }

    #[must_use]
    pub fn shows_top_menus(&self) -> bool {
        self.global_menu_use == GlobalMenuUse::Enabled
            && matches!(self.focused_menu_bar, FocusedMenuBar::Shown(_))
    }

    pub fn collapse(&mut self) {
        self.expansions.collapse();
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
        self.expansions.clear();
    }

    fn application_id(&self) -> &str {
        match &self.focused_window {
            FocusedWindowIdentity::Window {
                owner:
                    FocusedWindowOwner::ProcessAndApp { app_id, .. }
                    | FocusedWindowOwner::AppOnly { app_id },
                ..
            } => app_id,
            FocusedWindowIdentity::Window {
                owner: FocusedWindowOwner::ProcessOnly { .. } | FocusedWindowOwner::Unidentified,
                ..
            }
            | FocusedWindowIdentity::NoFocusedWindow => UNIDENTIFIED_APPLICATION,
        }
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
        match (&self.global_menu_use, &self.focused_menu_bar) {
            (GlobalMenuUse::Enabled, FocusedMenuBar::LookupFailed(lookup_failure)) => {
                Some(lookup_failure)
            }
            (GlobalMenuUse::Enabled | GlobalMenuUse::Disabled, _) => None,
        }
    }

    pub fn subscription(
        &self,
        buses: &DesktopBuses,
        source_demand: GroupSourceDemand,
    ) -> Subscription<GlobalMenuMessage> {
        if self.global_menu_use == GlobalMenuUse::Disabled {
            return Subscription::none();
        }
        let registrar = Subscription::run(appmenu_registrar_server::registrations)
            .map(GlobalMenuMessage::RegistrationsChanged);
        let menu_bar_query = MenuBarQuery {
            buses: buses.clone(),
            focused_window: self.focused_window.clone(),
            registered_menu_bars: self.registered_menu_bars.clone(),
        };
        let submenu_preparation = match &self.focused_menu_bar {
            FocusedMenuBar::Shown(shown_menu_bar) => match &shown_menu_bar.activation_route {
                MenuActivationRoute::DbusMenu(location) => {
                    source_demand.while_rows_shown(Subscription::run_with(
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
                    ))
                }
                MenuActivationRoute::GtkActions { .. } => Subscription::none(),
            },
            FocusedMenuBar::NotExported
            | FocusedMenuBar::NoFocusedWindow
            | FocusedMenuBar::LookupFailed(_) => Subscription::none(),
        };
        Subscription::batch([
            registrar,
            Subscription::run_with(menu_bar_query, focused_menu_bar_updates)
                .map(GlobalMenuMessage::MenuBarChanged),
            submenu_preparation,
        ])
    }

    pub fn update(&mut self, message: GlobalMenuMessage) -> Action {
        match message {
            GlobalMenuMessage::UseLoaded(global_menu_use) => {
                self.global_menu_use = global_menu_use;
                Action::None
            }
            GlobalMenuMessage::RegistrationsChanged(registered_menu_bars) => {
                self.registered_menu_bars = registered_menu_bars;
                Action::None
            }
            GlobalMenuMessage::MenuBarChanged(focused_menu_bar) => {
                self.adopt_menu_bar(focused_menu_bar);
                Action::None
            }
            GlobalMenuMessage::SubmenusPrepared => Action::None,
            GlobalMenuMessage::Interaction(interaction) => self.interact(interaction),
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
            self.expansions.track(
                shown_menu_bar
                    .top_menus
                    .iter()
                    .filter_map(|top_menu| top_menu.label.clone()),
            );
        }
        self.focused_menu_bar = focused_menu_bar;
    }

    fn interact(&mut self, interaction: GlobalMenuInteraction) -> Action {
        match interaction {
            GlobalMenuInteraction::ToggleUse => {
                self.global_menu_use = self.global_menu_use.toggled();
                if self.global_menu_use == GlobalMenuUse::Disabled {
                    self.registered_menu_bars.clear();
                    self.focused_menu_bar = FocusedMenuBar::NotExported;
                    self.expansions.clear();
                }
                Action::Run(logged_failure_task(
                    "singleton flag: save".to_owned(),
                    sqlite_singleton_flag::save(self.global_menu_use),
                ))
            }
            GlobalMenuInteraction::Expansion {
                top_menu_label,
                expansion_message,
            } => {
                self.expansions.update(&top_menu_label, expansion_message);
                Action::None
            }
            GlobalMenuInteraction::SwitchExpansion(expansion_message) => {
                self.switch_expansion.update(expansion_message);
                Action::None
            }
            GlobalMenuInteraction::ItemPressed(item_id) => self.item_activation(item_id),
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

    #[must_use]
    pub fn hover_card_for(&self, target: &GlobalMenuTarget, clock: Clock) -> Option<HoverCard> {
        match target {
            GlobalMenuTarget::Use => {
                Some(global_menu_use_toggle(self.global_menu_use).switch_hover_card(clock))
            }
            GlobalMenuTarget::LookupFailure => self.lookup_failure().map(|lookup_failure| {
                HoverCard::sourced(
                    MenuLookupFailureReading { lookup_failure }.hover_card(clock),
                    DeliveryTiming::LivePush,
                )
            }),
            GlobalMenuTarget::Entry { label_path, .. } => {
                let FocusedMenuBar::Shown(shown_menu_bar) = &self.focused_menu_bar else {
                    return None;
                };
                let entry_note = Section::note(label_path.clone());
                let body = match shown_menu_bar.menu_bar_match {
                    MenuBarMatch::ExactWindow | MenuBarMatch::OnlyProcessCandidate => {
                        HoverCardBody::from(entry_note)
                    }
                    MenuBarMatch::MostRecentProcessCandidate => HoverCardBody::sections([
                        entry_note,
                        Section::note(MOST_RECENT_OF_SEVERAL_NOTE.to_owned()),
                    ]),
                };
                Some(HoverCard::sourced(body, DeliveryTiming::LivePush))
            }
        }
    }

    pub fn switch_content(&self) -> WidgetGroupContent<'_, GlobalMenuInteraction> {
        let lookup_failure_row = self.lookup_failure().map(|lookup_failure| {
            GroupItem::reading(
                MenuLookupFailureReading { lookup_failure },
                GlobalMenuTarget::LookupFailure,
            )
        });
        WidgetGroupContent::expandable(
            WidgetGroupFamily::GlobalMenuSwitch,
            &self.switch_expansion,
            GlobalMenuInteraction::SwitchExpansion,
            [GroupItem::toggle(
                global_menu_use_toggle(self.global_menu_use),
                GlobalMenuTarget::Use,
                GlobalMenuInteraction::ToggleUse,
            )]
            .into_iter()
            .chain(lookup_failure_row),
        )
        .face(WidgetGroupFace::IconColoredBy {
            glyph: Phosphor::AppWindow(Weight::Regular).into(),
            reading: self,
        })
    }

    fn entry_item(&self, entry: FlattenedEntry<'_>) -> GroupItem<'_, GlobalMenuInteraction> {
        let node = entry.node;
        let icon = match (node.kind, &node.icon) {
            (MenuKind::Toggle(true), _) => GroupItemIcon::IconGlyph {
                glyph: Phosphor::Check(Weight::Regular).into(),
                state: ReadingDisplayState::Idle,
            },
            (_, Some(icon_source)) => {
                GroupItemIcon::Image(self.pixmap_handles.icon_handle(icon_source))
            }
            (_, None) => GroupItemIcon::IconGlyph {
                glyph: Phosphor::Dot(Weight::Regular).into(),
                state: ReadingDisplayState::Idle,
            },
        };
        let entry_item = GroupItem::composed_pinnable(
            ComposedGroupItemFace {
                name: entry.label_path.clone().into(),
                icon,
            },
            GlobalMenuTarget::Entry {
                app_id: self.application_id().to_owned(),
                label_path: entry.label_path,
            },
            GroupItemValue::Nothing,
        );
        if node.enabled {
            entry_item.on_press(RUN_ENTRY_HINT, GlobalMenuInteraction::ItemPressed(node.id))
        } else {
            entry_item
        }
    }

    #[must_use]
    pub fn group_contents(&self) -> Vec<WidgetGroupContent<'_, GlobalMenuInteraction>> {
        if self.global_menu_use == GlobalMenuUse::Disabled {
            return Vec::new();
        }
        let app_id = self.application_id();
        self.top_menus()
            .iter()
            .filter_map(|top_menu| {
                let top_menu_label = top_menu.label.clone()?;
                let expansion = self.expansions.get(&top_menu_label)?;
                let entry_items: Vec<_> = flattened_entries(top_menu)
                    .into_iter()
                    .map(|entry| self.entry_item(entry))
                    .collect();
                let expansion_label = top_menu_label.clone();
                Some(
                    WidgetGroupContent::expandable_instance(
                        WidgetGroupIdentity {
                            family: WidgetGroupFamily::GlobalMenu,
                            instance: WidgetGroupInstance::Durable(
                                format!("{app_id}/{top_menu_label}").into(),
                            ),
                        },
                        WidgetGroupTitle::Named(top_menu_label.clone().into()),
                        expansion,
                        move |expansion_message| GlobalMenuInteraction::Expansion {
                            top_menu_label: expansion_label.clone(),
                            expansion_message,
                        },
                        entry_items,
                    )
                    .face(WidgetGroupFace::Icon {
                        glyph: top_menu_glyph(&top_menu_label).into(),
                    }),
                )
            })
            .collect()
    }
}

impl GroupItemReading for GlobalMenuGroup {
    fn name(&self) -> &'static str {
        "Global application menu"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::AppWindow(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match self.lookup_failure() {
            Some(_) => ReadingDisplayState::Failed,
            None => ReadingDisplayState::Idle,
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections(self.lookup_failure().map(Section::error))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use sottomano_desktop_sources::desktop_windows_apps::{
        focused_window_menu_bar::ShownMenuBar,
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

    fn enabled_group_showing(activation_route: MenuActivationRoute) -> GlobalMenuGroup {
        let mut group = GlobalMenuGroup::default();
        let _ = group.update(GlobalMenuMessage::UseLoaded(GlobalMenuUse::Enabled));
        group.observe_focused_window(FocusedWindowIdentity::Window {
            window_id: 3,
            owner: FocusedWindowOwner::ProcessAndApp {
                process_id: 40,
                app_id: "org.example.Editor".to_owned(),
            },
        });
        let _ = group.update(GlobalMenuMessage::MenuBarChanged(FocusedMenuBar::Shown(
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
    fn nested_submenus_flatten_into_labelled_paths_without_separators() {
        let top_menus = file_menu();
        let label_paths: Vec<String> = flattened_entries(&top_menus[0])
            .into_iter()
            .map(|entry| entry.label_path)
            .collect();
        assert_eq!(label_paths, ["Recent › notes.txt", "Print", "Quit"]);
    }

    #[test]
    fn each_top_menu_becomes_its_own_widget_group() {
        let group = enabled_group_showing(MenuActivationRoute::DbusMenu(qt_menu_bar_location()));
        let group_contents = group.group_contents();
        assert_eq!(group_contents.len(), 1);
        assert!(group.shows_top_menus());
    }

    #[test]
    fn switched_off_group_shows_nothing() {
        let mut group =
            enabled_group_showing(MenuActivationRoute::DbusMenu(qt_menu_bar_location()));
        let _ = group.update(GlobalMenuMessage::Interaction(
            GlobalMenuInteraction::ToggleUse,
        ));
        assert_eq!(group.global_menu_use(), GlobalMenuUse::Disabled);
        assert!(group.group_contents().is_empty());
        assert!(!group.shows_top_menus());
    }

    #[test]
    fn group_starts_switched_off() {
        assert_eq!(
            GlobalMenuGroup::default().global_menu_use(),
            GlobalMenuUse::Disabled
        );
    }

    #[test]
    fn pressing_a_qt_entry_clicks_it_by_id() {
        let mut group =
            enabled_group_showing(MenuActivationRoute::DbusMenu(qt_menu_bar_location()));
        assert!(matches!(
            group.update(GlobalMenuMessage::Interaction(
                GlobalMenuInteraction::ItemPressed(QUIT_ITEM_ID),
            )),
            Action::Activate(GlobalMenuRequest::ClickDbusMenuItem {
                item_id: QUIT_ITEM_ID,
                ..
            })
        ));
    }

    #[test]
    fn pressing_a_gtk_entry_activates_its_action() {
        let mut group = enabled_group_showing(gtk_quit_route());
        let Action::Activate(GlobalMenuRequest::ActivateGtkAction { activation, .. }) = group
            .update(GlobalMenuMessage::Interaction(
                GlobalMenuInteraction::ItemPressed(QUIT_ITEM_ID),
            ))
        else {
            panic!("the quit entry activates a gtk action");
        };
        assert_eq!(activation.action_name, "quit");
    }

    #[test]
    fn entry_pin_keys_follow_labels_not_menu_ids() {
        let entry_target = |label_path: &str| {
            HoverTarget::from(GlobalMenuTarget::Entry {
                app_id: "org.example.Editor".to_owned(),
                label_path: label_path.to_owned(),
            })
            .stable_item_key()
        };
        assert_eq!(entry_target("File › Quit"), entry_target("File › Quit"));
        assert_ne!(entry_target("File › Quit"), entry_target("File › Print"));
    }

    #[test]
    fn menu_bar_survives_the_panel_taking_keyboard_focus() {
        let mut group = enabled_group_showing(gtk_quit_route());
        group.observe_focused_window(FocusedWindowIdentity::NoFocusedWindow);
        assert!(group.shows_top_menus());
        assert_eq!(group.group_contents().len(), 1);
    }

    #[test]
    fn focus_change_drops_the_previous_menu_bar() {
        let mut group = enabled_group_showing(gtk_quit_route());
        group.observe_focused_window(FocusedWindowIdentity::Window {
            window_id: 9,
            owner: FocusedWindowOwner::ProcessOnly { process_id: 9 },
        });
        assert!(group.group_contents().is_empty());
    }
}
