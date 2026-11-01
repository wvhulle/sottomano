mod menu_inspection;
mod sqlite_group_menu_pins;

use std::{collections::BTreeSet, mem::take};

use iced::Task;

use self::{menu_inspection::MenuInspection, sqlite_group_menu_pins::StoredGroupMenuPin};
use crate::{
    hover_tooltips::InspectedRow,
    widgets::{
        phosphor::{Phosphor, Weight},
        widget_group::GroupTitle,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupMenuKind {
    Hardware,
    Network,
    Agenda,
    Media,
    Display,
    Services,
    Session,
    Appearance,
    Behaviour,
}

impl GroupMenuKind {
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            GroupMenuKind::Hardware => "Hardware",
            GroupMenuKind::Network => "Network",
            GroupMenuKind::Agenda => "Agenda",
            GroupMenuKind::Media => "Media",
            GroupMenuKind::Display => "Display",
            GroupMenuKind::Services => "Services",
            GroupMenuKind::Session => "Session",
            GroupMenuKind::Appearance => "Appearance",
            GroupMenuKind::Behaviour => "Behaviour",
        }
    }

    #[must_use]
    pub const fn glyph(self) -> Phosphor {
        match self {
            GroupMenuKind::Hardware => Phosphor::Cpu(Weight::Regular),
            GroupMenuKind::Network => Phosphor::Network(Weight::Regular),
            GroupMenuKind::Agenda => Phosphor::CalendarBlank(Weight::Regular),
            GroupMenuKind::Media => Phosphor::MusicNotes(Weight::Regular),
            GroupMenuKind::Display => Phosphor::Monitor(Weight::Regular),
            GroupMenuKind::Services => Phosphor::Stack(Weight::Regular),
            GroupMenuKind::Session => Phosphor::UserCircle(Weight::Regular),
            GroupMenuKind::Appearance => Phosphor::SlidersHorizontal(Weight::Regular),
            GroupMenuKind::Behaviour => Phosphor::ToggleRight(Weight::Regular),
        }
    }

    const fn storage_key(self) -> &'static str {
        match self {
            GroupMenuKind::Hardware => "HardwareMenu",
            GroupMenuKind::Network => "NetworkMenu",
            GroupMenuKind::Agenda => "AgendaMenu",
            GroupMenuKind::Media => "MediaMenu",
            GroupMenuKind::Display => "DisplayMenu",
            GroupMenuKind::Services => "ServicesMenu",
            GroupMenuKind::Session => "SessionMenu",
            GroupMenuKind::Appearance => "AppearanceMenu",
            GroupMenuKind::Behaviour => "BehaviourMenu",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinnedEntry {
    Group {
        group_title: GroupTitle,
    },
    Item {
        group_title: GroupTitle,
        item_name: &'static str,
    },
}

impl PinnedEntry {
    fn stored_in(self, menu: GroupMenuKind) -> StoredGroupMenuPin {
        let group_menu = menu.storage_key().to_owned();
        match self {
            PinnedEntry::Group { group_title } => StoredGroupMenuPin::Group {
                group_menu,
                group_title: group_title.to_string(),
            },
            PinnedEntry::Item {
                group_title,
                item_name,
            } => StoredGroupMenuPin::Item {
                group_menu,
                group_title: group_title.to_string(),
                item_name: item_name.to_owned(),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub enum GroupMenuMessage {
    PinsLoaded(BTreeSet<StoredGroupMenuPin>),
    TogglePin {
        menu: GroupMenuKind,
        entry: PinnedEntry,
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
    pub fn is_pinned(&self, menu: GroupMenuKind, entry: PinnedEntry) -> bool {
        self.pinned.contains(&entry.stored_in(menu))
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
            GroupMenuMessage::TogglePin { menu, entry } => {
                let pin = entry.stored_in(menu);
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

#[cfg(test)]
mod tests {
    use super::{GroupMenuKind, GroupMenuMessage, GroupMenus, GroupTitle, PinnedEntry};

    const GPU_GROUP: PinnedEntry = PinnedEntry::Group {
        group_title: GroupTitle::Fixed("GPU"),
    };
    const GPU_LOAD_ITEM: PinnedEntry = PinnedEntry::Item {
        group_title: GroupTitle::Fixed("GPU"),
        item_name: "GPU load",
    };

    fn toggle_in_hardware(entry: PinnedEntry) -> GroupMenuMessage {
        GroupMenuMessage::TogglePin {
            menu: GroupMenuKind::Hardware,
            entry,
        }
    }

    #[test]
    fn toggling_twice_unpins_the_group() {
        let mut group_menus = GroupMenus::default();
        let _ = group_menus.update(toggle_in_hardware(GPU_GROUP));
        assert!(group_menus.is_pinned(GroupMenuKind::Hardware, GPU_GROUP));
        assert!(!group_menus.is_pinned(GroupMenuKind::Behaviour, GPU_GROUP));
        let _ = group_menus.update(toggle_in_hardware(GPU_GROUP));
        assert!(!group_menus.is_pinned(GroupMenuKind::Hardware, GPU_GROUP));
    }

    #[test]
    fn pinning_an_item_leaves_its_group_unpinned() {
        let mut group_menus = GroupMenus::default();
        let _ = group_menus.update(toggle_in_hardware(GPU_LOAD_ITEM));
        assert!(group_menus.is_pinned(GroupMenuKind::Hardware, GPU_LOAD_ITEM));
        assert!(!group_menus.is_pinned(GroupMenuKind::Hardware, GPU_GROUP));
        let _ = group_menus.update(toggle_in_hardware(GPU_GROUP));
        let _ = group_menus.update(toggle_in_hardware(GPU_LOAD_ITEM));
        assert!(!group_menus.is_pinned(GroupMenuKind::Hardware, GPU_LOAD_ITEM));
        assert!(group_menus.is_pinned(GroupMenuKind::Hardware, GPU_GROUP));
    }

    #[test]
    fn pinning_a_numbered_group_leaves_its_first_sibling_unpinned() {
        let first_port = PinnedEntry::Group {
            group_title: GroupTitle::numbered_from_second("USB Ethernet", 0),
        };
        let second_port = PinnedEntry::Group {
            group_title: GroupTitle::numbered_from_second("USB Ethernet", 1),
        };
        let mut group_menus = GroupMenus::default();
        let _ = group_menus.update(toggle_in_hardware(second_port));
        assert!(group_menus.is_pinned(GroupMenuKind::Hardware, second_port));
        assert!(!group_menus.is_pinned(GroupMenuKind::Hardware, first_port));
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
