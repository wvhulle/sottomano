use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

use zbus::zvariant::{self, OwnedValue, Structure, Value};

use super::{
    GtkActionGroups, GtkActionScope, GtkActionTarget, GtkMenuActivation, GtkMenuBar, GtkMenusProxy,
    gtk_actions_proxy::GtkActionDescription,
};
use crate::desktop_windows_apps::application_menu_nodes::{
    MenuItemIcon, MenuItemSensitivity, MenuKind, MenuLabel, MenuNode, ToggleCheck, ToggleMark,
    label_without_mnemonics,
};

const ROOT_MENU_GROUP: u32 = 0;
pub(super) const SECTION_LINK: &str = ":section";
pub(super) const SUBMENU_LINK: &str = ":submenu";

struct ScopedActionName {
    scope: GtkActionScope,
    action_name: String,
}

impl TryFrom<&str> for ScopedActionName {
    type Error = String;

    fn try_from(detailed_action: &str) -> Result<Self, Self::Error> {
        let Some((prefix, action_name)) = detailed_action.split_once('.') else {
            return Err(format!("{detailed_action} has no action group prefix"));
        };
        let scope = match prefix {
            "app" => GtkActionScope::Application,
            "win" => GtkActionScope::Window,
            _ => {
                return Err(format!(
                    "{detailed_action} belongs to an unexported action group"
                ));
            }
        };
        Ok(Self {
            scope,
            action_name: action_name.to_owned(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) struct GtkMenuKey {
    pub(super) group_id: u32,
    pub(super) menu_id: u32,
}

impl TryFrom<&OwnedValue> for GtkMenuKey {
    type Error = zvariant::Error;

    fn try_from(link_value: &OwnedValue) -> Result<Self, Self::Error> {
        let link_structure = link_value.downcast_ref::<Structure<'_>>()?;
        match link_structure.fields() {
            [Value::U32(group_id), Value::U32(menu_id)] => Ok(Self {
                group_id: *group_id,
                menu_id: *menu_id,
            }),
            _ => Err(zvariant::Error::IncorrectType),
        }
    }
}

pub(super) type GtkMenuItems = HashMap<GtkMenuKey, Vec<HashMap<String, OwnedValue>>>;
pub(super) type GtkActionDescriptions = HashMap<String, GtkActionDescription>;

pub(super) async fn loaded_menu_items(
    menus_proxy: &GtkMenusProxy<'_>,
) -> zbus::Result<GtkMenuItems> {
    let mut menu_items = GtkMenuItems::new();
    let mut requested_groups = BTreeSet::new();
    let mut pending_groups = vec![ROOT_MENU_GROUP];
    while !pending_groups.is_empty() {
        requested_groups.extend(pending_groups.iter().copied());
        for group_reply in menus_proxy.start(&pending_groups).await? {
            menu_items.insert(
                GtkMenuKey {
                    group_id: group_reply.group_id,
                    menu_id: group_reply.menu_id,
                },
                group_reply.items,
            );
        }
        pending_groups = menu_items
            .values()
            .flatten()
            .filter_map(linked_menu)
            .map(|linked_key| linked_key.group_id)
            .filter(|group_id| !requested_groups.contains(group_id))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
    }
    let started_groups: Vec<u32> = requested_groups.into_iter().collect();
    if let Err(end_failure) = menus_proxy.end(&started_groups).await {
        tracing::debug!("gtk menus: end subscription: {end_failure}");
    }
    Ok(menu_items)
}

fn linked_menu(menu_item: &HashMap<String, OwnedValue>) -> Option<GtkMenuKey> {
    [SECTION_LINK, SUBMENU_LINK]
        .into_iter()
        .find_map(|link_key| menu_item.get(link_key))
        .and_then(linked_menu_key)
}

fn linked_menu_key(link_value: &OwnedValue) -> Option<GtkMenuKey> {
    GtkMenuKey::try_from(link_value)
        .inspect_err(|link_failure| {
            tracing::warn!("gtk menus: invalid menu link {link_value:?}: {link_failure}");
        })
        .ok()
}

fn text_property(menu_item: &HashMap<String, OwnedValue>, property_name: &str) -> Option<String> {
    menu_item
        .get(property_name)
        .and_then(|property_value| {
            property_value
                .downcast_ref::<&str>()
                .inspect_err(|downcast_failure| {
                    tracing::warn!(
                        "gtk menus: {property_name} is not text {property_value:?}: \
                         {downcast_failure}"
                    );
                })
                .ok()
        })
        .map(str::to_owned)
}

struct GtkMenuTreeBuilder<'menus> {
    menu_items: &'menus GtkMenuItems,
    application_actions: &'menus GtkActionDescriptions,
    window_actions: &'menus GtkActionDescriptions,
    next_node_id: i32,
    activations: HashMap<i32, GtkMenuActivation>,
}

impl GtkMenuTreeBuilder<'_> {
    fn issue_node_id(&mut self) -> i32 {
        self.next_node_id += 1;
        self.next_node_id
    }

    fn menu_nodes(&mut self, menu_key: GtkMenuKey, depth_left: u8) -> Vec<MenuNode> {
        let Some(items) = self.menu_items.get(&menu_key) else {
            return Vec::new();
        };
        let mut menu_nodes = Vec::new();
        for menu_item in items {
            if let Some(section_key) = menu_item.get(SECTION_LINK).and_then(linked_menu_key) {
                let Some(section_depth_left) = depth_left.checked_sub(1) else {
                    continue;
                };
                let section_nodes = self.menu_nodes(section_key, section_depth_left);
                if !menu_nodes.is_empty() && !section_nodes.is_empty() {
                    menu_nodes.push(self.separator_node());
                }
                menu_nodes.extend(section_nodes);
                continue;
            }
            menu_nodes.extend(self.item_node(menu_item, depth_left));
        }
        menu_nodes
    }

    fn separator_node(&mut self) -> MenuNode {
        MenuNode {
            id: self.issue_node_id(),
            label: MenuLabel::Unlabelled,
            sensitivity: MenuItemSensitivity::Enabled,
            kind: MenuKind::Separator,
            icon: MenuItemIcon::NoIcon,
            children: Vec::new(),
        }
    }

    fn item_node(
        &mut self,
        menu_item: &HashMap<String, OwnedValue>,
        depth_left: u8,
    ) -> Option<MenuNode> {
        let label = text_property(menu_item, "label").map_or(MenuLabel::Unlabelled, |raw_label| {
            MenuLabel::Labelled(label_without_mnemonics(&raw_label))
        });
        let node_id = self.issue_node_id();
        if let Some(submenu_key) = menu_item.get(SUBMENU_LINK).and_then(linked_menu_key) {
            let children = if depth_left == 0 {
                Vec::new()
            } else {
                self.menu_nodes(submenu_key, depth_left - 1)
            };
            return Some(MenuNode {
                id: node_id,
                label,
                sensitivity: MenuItemSensitivity::Enabled,
                kind: MenuKind::Submenu,
                icon: MenuItemIcon::NoIcon,
                children,
            });
        }
        let detailed_action = text_property(menu_item, "action")?;
        let ScopedActionName { scope, action_name } =
            ScopedActionName::try_from(detailed_action.as_str())
                .inspect_err(|action_failure| {
                    tracing::warn!("gtk menus: invalid action {detailed_action}: {action_failure}");
                })
                .ok()?;
        let action_target = menu_item
            .get("target")
            .and_then(|target_value| {
                target_value
                    .try_clone()
                    .inspect_err(|clone_failure| {
                        tracing::warn!(
                            "gtk menus: cannot copy target of {detailed_action}: {clone_failure}"
                        );
                    })
                    .ok()
            })
            .map_or(GtkActionTarget::Untargeted, |target_value| {
                GtkActionTarget::Targeted(Arc::new(target_value))
            });
        let description = match scope {
            GtkActionScope::Application => self.application_actions.get(&action_name),
            GtkActionScope::Window => self.window_actions.get(&action_name),
        };
        let sensitivity = if description.is_some_and(|description| description.enabled) {
            MenuItemSensitivity::Enabled
        } else {
            MenuItemSensitivity::Disabled
        };
        let kind = description
            .and_then(|description| description.state.first())
            .map_or(MenuKind::Standard, |action_state| {
                toggle_kind(action_state, &action_target)
            });
        self.activations.insert(
            node_id,
            GtkMenuActivation {
                scope,
                action_name,
                action_target,
            },
        );
        Some(MenuNode {
            id: node_id,
            label,
            sensitivity,
            kind,
            icon: MenuItemIcon::NoIcon,
            children: Vec::new(),
        })
    }
}

fn toggle_kind(action_state: &OwnedValue, action_target: &GtkActionTarget) -> MenuKind {
    let action_state: &Value<'_> = action_state;
    let action_state = match action_state {
        Value::Value(boxed_state) => boxed_state.as_ref(),
        unboxed_state => unboxed_state,
    };
    match (action_state, action_target) {
        (Value::Bool(checked), GtkActionTarget::Untargeted) => MenuKind::Toggle {
            mark: ToggleMark::Checkbox,
            check: if *checked {
                ToggleCheck::Checked
            } else {
                ToggleCheck::Unchecked
            },
        },
        (current_state, GtkActionTarget::Targeted(target_value)) => {
            let target_value: &Value<'_> = target_value;
            MenuKind::Toggle {
                mark: ToggleMark::Radio,
                check: if current_state == target_value {
                    ToggleCheck::Checked
                } else {
                    ToggleCheck::Unchecked
                },
            }
        }
        (_, GtkActionTarget::Untargeted) => MenuKind::Standard,
    }
}

const MAXIMUM_MENU_DEPTH: u8 = 8;

pub(super) fn gtk_menu_bar(
    menu_items: &GtkMenuItems,
    application_actions: &GtkActionDescriptions,
    window_actions: &GtkActionDescriptions,
    action_groups: GtkActionGroups,
) -> GtkMenuBar {
    let mut tree_builder = GtkMenuTreeBuilder {
        menu_items,
        application_actions,
        window_actions,
        next_node_id: 0,
        activations: HashMap::new(),
    };
    let top_menus = tree_builder.menu_nodes(
        GtkMenuKey {
            group_id: ROOT_MENU_GROUP,
            menu_id: 0,
        },
        MAXIMUM_MENU_DEPTH,
    );
    GtkMenuBar {
        top_menus,
        activations: tree_builder.activations,
        action_groups,
    }
}
