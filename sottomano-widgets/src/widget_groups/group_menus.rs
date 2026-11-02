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
    group_source_demand::GroupSourceDemand, menu_inspection::GroupMenuInspection,
    sqlite_group_menu_pins::StoredGroupMenuPin,
};
use crate::{
    hover_tooltips::InspectedRow,
    widgets::{
        phosphor::{Phosphor, Weight},
        widget_group::WidgetGroupTitle,
    },
};

const MENU_SOURCE_LINGER: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupMenuTopic {
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

impl GroupMenuTopic {
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            GroupMenuTopic::Hardware => "Hardware",
            GroupMenuTopic::Network => "Network",
            GroupMenuTopic::Agenda => "Agenda",
            GroupMenuTopic::Media => "Media",
            GroupMenuTopic::Display => "Display",
            GroupMenuTopic::Services => "Services",
            GroupMenuTopic::Session => "Session",
            GroupMenuTopic::Appearance => "Appearance",
            GroupMenuTopic::Behaviour => "Behaviour",
        }
    }

    #[must_use]
    pub const fn glyph(self) -> Phosphor {
        match self {
            GroupMenuTopic::Hardware => Phosphor::Cpu(Weight::Regular),
            GroupMenuTopic::Network => Phosphor::Network(Weight::Regular),
            GroupMenuTopic::Agenda => Phosphor::CalendarBlank(Weight::Regular),
            GroupMenuTopic::Media => Phosphor::MusicNotes(Weight::Regular),
            GroupMenuTopic::Display => Phosphor::Monitor(Weight::Regular),
            GroupMenuTopic::Services => Phosphor::Stack(Weight::Regular),
            GroupMenuTopic::Session => Phosphor::UserCircle(Weight::Regular),
            GroupMenuTopic::Appearance => Phosphor::SlidersHorizontal(Weight::Regular),
            GroupMenuTopic::Behaviour => Phosphor::ToggleRight(Weight::Regular),
        }
    }

    const fn storage_key(self) -> &'static str {
        match self {
            GroupMenuTopic::Hardware => "HardwareMenu",
            GroupMenuTopic::Network => "NetworkMenu",
            GroupMenuTopic::Agenda => "AgendaMenu",
            GroupMenuTopic::Media => "MediaMenu",
            GroupMenuTopic::Display => "DisplayMenu",
            GroupMenuTopic::Services => "ServicesMenu",
            GroupMenuTopic::Session => "SessionMenu",
            GroupMenuTopic::Appearance => "AppearanceMenu",
            GroupMenuTopic::Behaviour => "BehaviourMenu",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupMenuPin {
    Group {
        group_title: WidgetGroupTitle,
    },
    Item {
        group_title: WidgetGroupTitle,
        item_name: &'static str,
    },
}

impl GroupMenuPin {
    fn stored_in(self, menu: GroupMenuTopic) -> StoredGroupMenuPin {
        let group_menu = menu.storage_key().to_owned();
        match self {
            GroupMenuPin::Group { group_title } => StoredGroupMenuPin::Group {
                group_menu,
                group_title: group_title.to_string(),
            },
            GroupMenuPin::Item {
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
        menu: GroupMenuTopic,
        entry: GroupMenuPin,
    },
    PinSaved(bool),
    InspectMember {
        menu: GroupMenuTopic,
        member: usize,
    },
    InspectRow {
        menu: GroupMenuTopic,
        row: InspectedRow,
    },
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertPromotedGroup {
    pub menu: GroupMenuTopic,
    pub group_title: WidgetGroupTitle,
}

#[derive(Debug, Clone, Copy)]
struct RecentlyOpenMenu {
    menu: GroupMenuTopic,
    last_open_at: Instant,
}

#[derive(Default)]
pub struct GroupMenuState {
    pinned: BTreeSet<StoredGroupMenuPin>,
    inspection: GroupMenuInspection,
    recently_open_menus: Vec<RecentlyOpenMenu>,
    promoted_groups: Vec<AlertPromotedGroup>,
}

impl GroupMenuState {
    pub fn load() -> Task<GroupMenuMessage> {
        Task::perform(sqlite_group_menu_pins::load(), GroupMenuMessage::PinsLoaded)
    }

    #[must_use]
    pub fn is_pinned(&self, menu: GroupMenuTopic, entry: GroupMenuPin) -> bool {
        self.pinned.contains(&entry.stored_in(menu))
    }

    pub fn track_source_demand(
        &mut self,
        open_menu: Option<GroupMenuTopic>,
        promoted_groups: Vec<AlertPromotedGroup>,
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
        menu: GroupMenuTopic,
        group_titles: &[WidgetGroupTitle],
    ) -> GroupSourceDemand<'_> {
        let menu_key = menu.storage_key();
        let stored_titles: Vec<String> = group_titles.iter().map(ToString::to_string).collect();
        let menu_open = self
            .recently_open_menus
            .iter()
            .any(|recent| recent.menu == menu);
        let on_panel = group_titles.iter().any(|group_title| {
            self.is_pinned(
                menu,
                GroupMenuPin::Group {
                    group_title: group_title.clone(),
                },
            ) || self.promoted_groups.contains(&AlertPromotedGroup {
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
        GroupSourceDemand::in_menu(menu_open, on_panel, pinned_item_names)
    }

    #[must_use]
    pub fn open_member(&self, menu: GroupMenuTopic) -> Option<usize> {
        self.inspection.open_member(menu)
    }

    #[must_use]
    pub fn inspected_row(&self, menu: GroupMenuTopic) -> Option<&InspectedRow> {
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
                self.inspection = GroupMenuInspection::Closed;
                Task::none()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{
        AlertPromotedGroup, GroupMenuMessage, GroupMenuPin, GroupMenuState, GroupMenuTopic,
        GroupSourceDemand, MENU_SOURCE_LINGER, WidgetGroupTitle,
    };
    use crate::clock::Clock;

    const GPU_GROUP: GroupMenuPin = GroupMenuPin::Group {
        group_title: WidgetGroupTitle::fixed("GPU"),
    };
    const GPU_LOAD_ITEM: GroupMenuPin = GroupMenuPin::Item {
        group_title: WidgetGroupTitle::fixed("GPU"),
        item_name: "GPU load",
    };

    fn toggle_in_hardware(entry: GroupMenuPin) -> GroupMenuMessage {
        GroupMenuMessage::TogglePin {
            menu: GroupMenuTopic::Hardware,
            entry,
        }
    }

    #[test]
    fn toggling_twice_unpins_the_group() {
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(toggle_in_hardware(GPU_GROUP));
        assert!(group_menus.is_pinned(GroupMenuTopic::Hardware, GPU_GROUP));
        assert!(!group_menus.is_pinned(GroupMenuTopic::Behaviour, GPU_GROUP));
        let _ = group_menus.update(toggle_in_hardware(GPU_GROUP));
        assert!(!group_menus.is_pinned(GroupMenuTopic::Hardware, GPU_GROUP));
    }

    #[test]
    fn pinning_an_item_leaves_its_group_unpinned() {
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(toggle_in_hardware(GPU_LOAD_ITEM));
        assert!(group_menus.is_pinned(GroupMenuTopic::Hardware, GPU_LOAD_ITEM));
        assert!(!group_menus.is_pinned(GroupMenuTopic::Hardware, GPU_GROUP));
        let _ = group_menus.update(toggle_in_hardware(GPU_GROUP));
        let _ = group_menus.update(toggle_in_hardware(GPU_LOAD_ITEM));
        assert!(!group_menus.is_pinned(GroupMenuTopic::Hardware, GPU_LOAD_ITEM));
        assert!(group_menus.is_pinned(GroupMenuTopic::Hardware, GPU_GROUP));
    }

    fn gpu_demand(group_menus: &GroupMenuState) -> GroupSourceDemand<'_> {
        group_menus.source_demand(GroupMenuTopic::Hardware, &[WidgetGroupTitle::fixed("GPU")])
    }

    fn collapsed_on_panel() -> GroupSourceDemand<'static> {
        GroupSourceDemand::on_panel()
    }

    #[test]
    fn unpinning_an_expanded_group_drops_its_demand() {
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(toggle_in_hardware(GPU_GROUP));
        assert_eq!(gpu_demand(&group_menus), collapsed_on_panel());
        let _ = group_menus.update(toggle_in_hardware(GPU_GROUP));
        assert_ne!(
            gpu_demand(&group_menus).expanded_when(true),
            collapsed_on_panel().expanded_when(true)
        );
    }

    #[test]
    fn promoted_and_pinned_groups_have_the_same_demand() {
        let mut group_menus = GroupMenuState::default();
        group_menus.track_source_demand(
            None,
            vec![AlertPromotedGroup {
                menu: GroupMenuTopic::Hardware,
                group_title: WidgetGroupTitle::fixed("GPU"),
            }],
            Clock::real().instant,
        );
        assert_eq!(gpu_demand(&group_menus), collapsed_on_panel());
        group_menus.track_source_demand(None, Vec::new(), Clock::real().instant);
        assert_ne!(gpu_demand(&group_menus), collapsed_on_panel());
    }

    #[test]
    fn pinned_item_demand_is_limited_to_its_own_group_and_menu() {
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(toggle_in_hardware(GPU_LOAD_ITEM));
        let other_group = group_menus.source_demand(
            GroupMenuTopic::Hardware,
            &[WidgetGroupTitle::fixed("Compute")],
        );
        let other_menu =
            group_menus.source_demand(GroupMenuTopic::Network, &[WidgetGroupTitle::fixed("GPU")]);
        assert_ne!(gpu_demand(&group_menus), other_group);
        assert_eq!(other_group, other_menu);
    }

    #[test]
    fn a_pinned_numbered_sibling_demands_the_source_its_family_shares() {
        let second_usb_port = GroupMenuPin::Group {
            group_title: WidgetGroupTitle::numbered_from_second("USB Ethernet", 1),
        };
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(toggle_in_hardware(second_usb_port));
        assert_eq!(
            group_menus.source_demand(
                GroupMenuTopic::Hardware,
                &WidgetGroupTitle::numbered_siblings("USB Ethernet")
            ),
            collapsed_on_panel()
        );
        assert_ne!(
            group_menus.source_demand(
                GroupMenuTopic::Hardware,
                &WidgetGroupTitle::numbered_siblings("Ethernet")
            ),
            collapsed_on_panel()
        );
    }

    #[test]
    fn a_promoted_numbered_sibling_demands_the_source_its_family_shares() {
        let mut group_menus = GroupMenuState::default();
        group_menus.track_source_demand(
            None,
            vec![AlertPromotedGroup {
                menu: GroupMenuTopic::Network,
                group_title: WidgetGroupTitle::numbered_from_second("Wi-Fi", 2),
            }],
            Clock::real().instant,
        );
        assert_eq!(
            group_menus.source_demand(
                GroupMenuTopic::Network,
                &WidgetGroupTitle::numbered_siblings("Wi-Fi")
            ),
            collapsed_on_panel()
        );
    }

    #[test]
    fn a_title_that_only_starts_with_another_is_a_separate_group() {
        let unrelated_group = GroupMenuPin::Group {
            group_title: WidgetGroupTitle::fixed("GPU 2 spare"),
        };
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(toggle_in_hardware(unrelated_group));
        assert_ne!(
            group_menus.source_demand(
                GroupMenuTopic::Hardware,
                &WidgetGroupTitle::numbered_siblings("GPU")
            ),
            collapsed_on_panel()
        );
    }

    #[test]
    fn demand_outlasts_the_open_menu_for_the_linger_only() {
        let mut group_menus = GroupMenuState::default();
        let opened_at = Clock::real().instant;
        group_menus.track_source_demand(Some(GroupMenuTopic::Hardware), Vec::new(), opened_at);
        let while_open = gpu_demand(&group_menus);
        assert_ne!(while_open, collapsed_on_panel());
        let shortly_after = opened_at + Duration::from_secs(1);
        group_menus.track_source_demand(None, Vec::new(), shortly_after);
        assert_eq!(
            gpu_demand(&group_menus),
            group_menus_with_open_hardware()
                .source_demand(GroupMenuTopic::Hardware, &[WidgetGroupTitle::fixed("GPU")])
        );
        group_menus.track_source_demand(None, Vec::new(), opened_at + MENU_SOURCE_LINGER);
        assert_eq!(
            gpu_demand(&group_menus),
            GroupMenuState::default()
                .source_demand(GroupMenuTopic::Hardware, &[WidgetGroupTitle::fixed("GPU")])
        );
    }

    fn group_menus_with_open_hardware() -> GroupMenuState {
        let mut group_menus = GroupMenuState::default();
        group_menus.track_source_demand(
            Some(GroupMenuTopic::Hardware),
            Vec::new(),
            Clock::real().instant,
        );
        group_menus
    }

    #[test]
    fn pinning_a_numbered_group_leaves_its_first_sibling_unpinned() {
        let first_port = GroupMenuPin::Group {
            group_title: WidgetGroupTitle::numbered_from_second("USB Ethernet", 0),
        };
        let second_port = GroupMenuPin::Group {
            group_title: WidgetGroupTitle::numbered_from_second("USB Ethernet", 1),
        };
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(toggle_in_hardware(second_port.clone()));
        assert!(group_menus.is_pinned(GroupMenuTopic::Hardware, second_port));
        assert!(!group_menus.is_pinned(GroupMenuTopic::Hardware, first_port));
    }

    #[test]
    fn closing_forgets_the_open_member() {
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(GroupMenuMessage::InspectMember {
            menu: GroupMenuTopic::Hardware,
            member: 1,
        });
        let _ = group_menus.update(GroupMenuMessage::Closed);
        assert_eq!(group_menus.open_member(GroupMenuTopic::Hardware), None);
    }
}
