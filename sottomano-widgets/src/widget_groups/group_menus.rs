pub mod group_source_demand;
mod menu_inspection;
mod sqlite_group_menu_pins;

use std::{
    collections::BTreeSet,
    mem::take,
    time::{Duration, Instant},
};

use iced::Task;

use self::{
    group_source_demand::GroupSourceDemand, menu_inspection::MenuInspection,
    sqlite_group_menu_pins::StoredGroupMenuPin,
};
use crate::{
    hover_tooltips::InspectedRow,
    widgets::{
        phosphor::{Phosphor, Weight},
        widget_group::GroupTitle,
    },
};

const MENU_SOURCE_LINGER: Duration = Duration::from_secs(30);

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

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotedGroup {
    pub menu: GroupMenuKind,
    pub group_title: GroupTitle,
}

#[derive(Debug, Clone, Copy)]
struct RecentlyOpenMenu {
    menu: GroupMenuKind,
    last_open_at: Instant,
}

#[derive(Default)]
pub struct GroupMenus {
    pinned: BTreeSet<StoredGroupMenuPin>,
    inspection: MenuInspection,
    recently_open_menus: Vec<RecentlyOpenMenu>,
    promoted_groups: Vec<PromotedGroup>,
}

impl GroupMenus {
    pub fn load() -> Task<GroupMenuMessage> {
        Task::perform(sqlite_group_menu_pins::load(), GroupMenuMessage::PinsLoaded)
    }

    #[must_use]
    pub fn is_pinned(&self, menu: GroupMenuKind, entry: PinnedEntry) -> bool {
        self.pinned.contains(&entry.stored_in(menu))
    }

    pub fn track_source_demand(
        &mut self,
        open_menu: Option<GroupMenuKind>,
        promoted_groups: Vec<PromotedGroup>,
        now: Instant,
    ) {
        self.promoted_groups = promoted_groups;
        self.recently_open_menus.retain(|recent| {
            Some(recent.menu) != open_menu
                && now.duration_since(recent.last_open_at) < MENU_SOURCE_LINGER
        });
        if let Some(menu) = open_menu {
            self.recently_open_menus.push(RecentlyOpenMenu {
                menu,
                last_open_at: now,
            });
        }
    }

    #[must_use]
    pub fn source_demand(
        &self,
        menu: GroupMenuKind,
        group_titles: &[GroupTitle],
    ) -> GroupSourceDemand<'_> {
        let menu_key = menu.storage_key();
        let stored_titles: Vec<String> = group_titles.iter().map(ToString::to_string).collect();
        let menu_open = self
            .recently_open_menus
            .iter()
            .any(|recent| recent.menu == menu);
        let on_bar = group_titles.iter().any(|group_title| {
            self.is_pinned(
                menu,
                PinnedEntry::Group {
                    group_title: group_title.clone(),
                },
            ) || self.promoted_groups.contains(&PromotedGroup {
                menu,
                group_title: group_title.clone(),
            })
        });
        let pinned_item_names = self.pinned.iter().filter_map(|pin| match pin {
            StoredGroupMenuPin::Item {
                group_menu,
                group_title,
                item_name,
            } if group_menu == menu_key && stored_titles.contains(group_title) => {
                Some(item_name.as_str())
            }
            StoredGroupMenuPin::Group { .. } | StoredGroupMenuPin::Item { .. } => None,
        });
        GroupSourceDemand::in_menu(menu_open, on_bar, pinned_item_names)
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
    use std::time::Duration;

    use super::{
        GroupMenuKind, GroupMenuMessage, GroupMenus, GroupSourceDemand, GroupTitle,
        MENU_SOURCE_LINGER, PinnedEntry, PromotedGroup,
    };
    use crate::clock::Clock;

    const GPU_GROUP: PinnedEntry = PinnedEntry::Group {
        group_title: GroupTitle::fixed("GPU"),
    };
    const GPU_LOAD_ITEM: PinnedEntry = PinnedEntry::Item {
        group_title: GroupTitle::fixed("GPU"),
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

    fn gpu_demand(group_menus: &GroupMenus) -> GroupSourceDemand<'_> {
        group_menus.source_demand(GroupMenuKind::Hardware, &[GroupTitle::fixed("GPU")])
    }

    fn collapsed_on_bar() -> GroupSourceDemand<'static> {
        GroupSourceDemand::on_bar()
    }

    #[test]
    fn unpinning_an_expanded_group_drops_its_demand() {
        let mut group_menus = GroupMenus::default();
        let _ = group_menus.update(toggle_in_hardware(GPU_GROUP));
        assert_eq!(gpu_demand(&group_menus), collapsed_on_bar());
        let _ = group_menus.update(toggle_in_hardware(GPU_GROUP));
        assert_ne!(
            gpu_demand(&group_menus).expanded_when(true),
            collapsed_on_bar().expanded_when(true)
        );
    }

    #[test]
    fn promoted_and_pinned_groups_have_the_same_demand() {
        let mut group_menus = GroupMenus::default();
        group_menus.track_source_demand(
            None,
            vec![PromotedGroup {
                menu: GroupMenuKind::Hardware,
                group_title: GroupTitle::fixed("GPU"),
            }],
            Clock::real().instant,
        );
        assert_eq!(gpu_demand(&group_menus), collapsed_on_bar());
        group_menus.track_source_demand(None, Vec::new(), Clock::real().instant);
        assert_ne!(gpu_demand(&group_menus), collapsed_on_bar());
    }

    #[test]
    fn pinned_item_demand_is_limited_to_its_own_group_and_menu() {
        let mut group_menus = GroupMenus::default();
        let _ = group_menus.update(toggle_in_hardware(GPU_LOAD_ITEM));
        let other_group =
            group_menus.source_demand(GroupMenuKind::Hardware, &[GroupTitle::fixed("Compute")]);
        let other_menu =
            group_menus.source_demand(GroupMenuKind::Network, &[GroupTitle::fixed("GPU")]);
        assert_ne!(gpu_demand(&group_menus), other_group);
        assert_eq!(other_group, other_menu);
    }

    #[test]
    fn a_pinned_numbered_sibling_demands_the_source_its_family_shares() {
        let second_usb_port = PinnedEntry::Group {
            group_title: GroupTitle::numbered_from_second("USB Ethernet", 1),
        };
        let mut group_menus = GroupMenus::default();
        let _ = group_menus.update(toggle_in_hardware(second_usb_port));
        assert_eq!(
            group_menus.source_demand(
                GroupMenuKind::Hardware,
                &GroupTitle::numbered_siblings("USB Ethernet")
            ),
            collapsed_on_bar()
        );
        assert_ne!(
            group_menus.source_demand(
                GroupMenuKind::Hardware,
                &GroupTitle::numbered_siblings("Ethernet")
            ),
            collapsed_on_bar()
        );
    }

    #[test]
    fn a_promoted_numbered_sibling_demands_the_source_its_family_shares() {
        let mut group_menus = GroupMenus::default();
        group_menus.track_source_demand(
            None,
            vec![PromotedGroup {
                menu: GroupMenuKind::Network,
                group_title: GroupTitle::numbered_from_second("Wi-Fi", 2),
            }],
            Clock::real().instant,
        );
        assert_eq!(
            group_menus.source_demand(
                GroupMenuKind::Network,
                &GroupTitle::numbered_siblings("Wi-Fi")
            ),
            collapsed_on_bar()
        );
    }

    #[test]
    fn a_title_that_only_starts_with_another_is_a_separate_group() {
        let unrelated_group = PinnedEntry::Group {
            group_title: GroupTitle::fixed("GPU 2 spare"),
        };
        let mut group_menus = GroupMenus::default();
        let _ = group_menus.update(toggle_in_hardware(unrelated_group));
        assert_ne!(
            group_menus.source_demand(
                GroupMenuKind::Hardware,
                &GroupTitle::numbered_siblings("GPU")
            ),
            collapsed_on_bar()
        );
    }

    #[test]
    fn demand_outlasts_the_open_menu_for_the_linger_only() {
        let mut group_menus = GroupMenus::default();
        let opened_at = Clock::real().instant;
        group_menus.track_source_demand(Some(GroupMenuKind::Hardware), Vec::new(), opened_at);
        let while_open = gpu_demand(&group_menus);
        assert_ne!(while_open, collapsed_on_bar());
        let shortly_after = opened_at + Duration::from_secs(1);
        group_menus.track_source_demand(None, Vec::new(), shortly_after);
        assert_eq!(
            gpu_demand(&group_menus),
            group_menus_with_open_hardware()
                .source_demand(GroupMenuKind::Hardware, &[GroupTitle::fixed("GPU")])
        );
        group_menus.track_source_demand(None, Vec::new(), opened_at + MENU_SOURCE_LINGER);
        assert_eq!(
            gpu_demand(&group_menus),
            GroupMenus::default()
                .source_demand(GroupMenuKind::Hardware, &[GroupTitle::fixed("GPU")])
        );
    }

    fn group_menus_with_open_hardware() -> GroupMenus {
        let mut group_menus = GroupMenus::default();
        group_menus.track_source_demand(
            Some(GroupMenuKind::Hardware),
            Vec::new(),
            Clock::real().instant,
        );
        group_menus
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
        let _ = group_menus.update(toggle_in_hardware(second_port.clone()));
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
