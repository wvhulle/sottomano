mod menu_inspection;
mod sqlite_group_menu_pins;

use std::{collections::BTreeSet, mem::take};

use iced::Task;

use self::{menu_inspection::MenuInspection, sqlite_group_menu_pins::StoredGroupMenuPin};
use crate::hover_tooltips::InspectedRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupMenuKind {
    Hardware,
    Network,
    Services,
    Appearance,
    Behaviour,
}

impl GroupMenuKind {
    const fn storage_key(self) -> &'static str {
        match self {
            GroupMenuKind::Hardware => "HardwareMenu",
            GroupMenuKind::Network => "NetworkMenu",
            GroupMenuKind::Services => "ServicesMenu",
            GroupMenuKind::Appearance => "AppearanceMenu",
            GroupMenuKind::Behaviour => "BehaviourMenu",
        }
    }
}

#[derive(Debug, Clone)]
pub enum GroupMenuMessage {
    PinsLoaded(BTreeSet<StoredGroupMenuPin>),
    TogglePin {
        menu: GroupMenuKind,
        group_title: &'static str,
    },
    PinSaved(bool),
    InspectMember {
        menu: GroupMenuKind,
        member: usize,
    },
    InspectRow {
        menu: GroupMenuKind,
        row: InspectedRow,
    },
    Closed,
}

#[derive(Default)]
pub struct GroupMenus {
    pinned: BTreeSet<StoredGroupMenuPin>,
    inspection: MenuInspection,
}

impl GroupMenus {
    pub fn load() -> Task<GroupMenuMessage> {
        Task::perform(sqlite_group_menu_pins::load(), GroupMenuMessage::PinsLoaded)
    }

    #[must_use]
    pub fn is_pinned(&self, menu: GroupMenuKind, group_title: &str) -> bool {
        self.pinned.contains(&stored_pin(menu, group_title))
    }

    #[must_use]
    pub fn open_member(&self, menu: GroupMenuKind) -> Option<usize> {
        self.inspection.open_member(menu)
    }

    #[must_use]
    pub fn inspected_row(&self, menu: GroupMenuKind) -> Option<&InspectedRow> {
        self.inspection.inspected_row(menu)
    }

    pub fn update(&mut self, message: GroupMenuMessage) -> Task<GroupMenuMessage> {
        match message {
            GroupMenuMessage::PinsLoaded(pinned) => {
                self.pinned = pinned;
                Task::none()
            }
            GroupMenuMessage::TogglePin { menu, group_title } => {
                let pin = stored_pin(menu, group_title);
                let pinned = !self.pinned.remove(&pin);
                if pinned {
                    self.pinned.insert(pin.clone());
                }
                Task::perform(
                    sqlite_group_menu_pins::save(pin, pinned),
                    GroupMenuMessage::PinSaved,
                )
            }
            GroupMenuMessage::PinSaved(saved) => {
                if !saved {
                    tracing::warn!("group menu pins: the pin change was not saved");
                }
                Task::none()
            }
            GroupMenuMessage::InspectMember { menu, member } => {
                self.inspection = take(&mut self.inspection).inspecting_member(menu, member);
                Task::none()
            }
            GroupMenuMessage::InspectRow { menu, row } => {
                self.inspection = take(&mut self.inspection).inspecting_row(menu, row);
                Task::none()
            }
            GroupMenuMessage::Closed => {
                self.inspection = MenuInspection::Closed;
                Task::none()
            }
        }
    }
}

fn stored_pin(menu: GroupMenuKind, group_title: &str) -> StoredGroupMenuPin {
    StoredGroupMenuPin {
        group_menu: menu.storage_key().to_owned(),
        group_title: group_title.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{GroupMenuKind, GroupMenuMessage, GroupMenus};

    #[test]
    fn toggling_twice_unpins_the_group() {
        let mut group_menus = GroupMenus::default();
        let toggle_gpu = || GroupMenuMessage::TogglePin {
            menu: GroupMenuKind::Hardware,
            group_title: "GPU",
        };
        let _ = group_menus.update(toggle_gpu());
        assert!(group_menus.is_pinned(GroupMenuKind::Hardware, "GPU"));
        assert!(!group_menus.is_pinned(GroupMenuKind::Behaviour, "GPU"));
        let _ = group_menus.update(toggle_gpu());
        assert!(!group_menus.is_pinned(GroupMenuKind::Hardware, "GPU"));
    }

    #[test]
    fn closing_forgets_the_open_member() {
        let mut group_menus = GroupMenus::default();
        let _ = group_menus.update(GroupMenuMessage::InspectMember {
            menu: GroupMenuKind::Hardware,
            member: 1,
        });
        let _ = group_menus.update(GroupMenuMessage::Closed);
        assert_eq!(group_menus.open_member(GroupMenuKind::Hardware), None);
    }
}
