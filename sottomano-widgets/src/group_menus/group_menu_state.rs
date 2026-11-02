use std::{
    collections::BTreeSet,
    mem::take,
    time::{Duration, Instant},
};

use iced::Task;

use crate::{
    group_menus::{
        group_menu_inspection::GroupMenuInspection, group_source_demand::GroupSourceDemand,
        sqlite_group_menu_pins, sqlite_group_menu_pins::StoredGroupMenuPin,
    },
    hover_cards::HoverTarget,
    widget_groups::widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity},
    widgets::phosphor::{Phosphor, Weight},
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
    pub const ALL: [GroupMenuTopic; 9] = [
        GroupMenuTopic::Hardware,
        GroupMenuTopic::Network,
        GroupMenuTopic::Agenda,
        GroupMenuTopic::Media,
        GroupMenuTopic::Display,
        GroupMenuTopic::Services,
        GroupMenuTopic::Session,
        GroupMenuTopic::Appearance,
        GroupMenuTopic::Behaviour,
    ];

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
            GroupMenuTopic::Hardware => "hardware",
            GroupMenuTopic::Network => "network",
            GroupMenuTopic::Agenda => "agenda",
            GroupMenuTopic::Media => "media",
            GroupMenuTopic::Display => "display",
            GroupMenuTopic::Services => "services",
            GroupMenuTopic::Session => "session",
            GroupMenuTopic::Appearance => "appearance",
            GroupMenuTopic::Behaviour => "behaviour",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupMenuPin {
    Group(WidgetGroupIdentity),
    Item {
        group: WidgetGroupIdentity,
        item_key: String,
    },
}

impl GroupMenuPin {
    fn stored_in(self, menu: GroupMenuTopic) -> StoredGroupMenuPin {
        let menu_topic = menu.storage_key().to_owned();
        match self {
            GroupMenuPin::Group(group) => StoredGroupMenuPin::Group {
                menu_topic,
                group_family: group.family.stored_name().to_owned(),
                group_instance: group.instance.stored_name().to_owned(),
            },
            GroupMenuPin::Item { group, item_key } => StoredGroupMenuPin::Item {
                menu_topic,
                group_family: group.family.stored_name().to_owned(),
                group_instance: group.instance.stored_name().to_owned(),
                item_key,
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
    RevealMenuGroup {
        menu: GroupMenuTopic,
        group_identity: WidgetGroupIdentity,
    },
    InspectRow {
        menu: GroupMenuTopic,
        target: HoverTarget,
    },
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertPromotedGroup {
    pub menu: GroupMenuTopic,
    pub group: WidgetGroupIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenGroupMenu {
    pub topic: GroupMenuTopic,
    pub families: Vec<WidgetGroupFamily>,
}

#[derive(Debug, Clone)]
struct RecentlyOpenMenu {
    menu: OpenGroupMenu,
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
        open_menu: Option<OpenGroupMenu>,
        promoted_groups: Vec<AlertPromotedGroup>,
        now: Instant,
    ) {
        self.promoted_groups = promoted_groups;
        let open_topic = open_menu.as_ref().map(|menu| menu.topic);
        self.recently_open_menus.retain(|recent| {
            Some(recent.menu.topic) != open_topic
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
    pub fn source_demand(&self, family: WidgetGroupFamily) -> GroupSourceDemand {
        let stored_family = family.stored_name();
        let menu_open = self
            .recently_open_menus
            .iter()
            .any(|recent| recent.menu.families.contains(&family));
        let group_pinned = self.pinned.iter().any(|pin| match pin {
            StoredGroupMenuPin::Group { group_family, .. } => group_family == stored_family,
            StoredGroupMenuPin::Item { .. } => false,
        });
        let group_promoted = self
            .promoted_groups
            .iter()
            .any(|promoted| promoted.group.family == family);
        let item_pinned = self.pinned.iter().any(|pin| match pin {
            StoredGroupMenuPin::Item { group_family, .. } => group_family == stored_family,
            StoredGroupMenuPin::Group { .. } => false,
        });
        GroupSourceDemand::in_menu(menu_open, group_pinned || group_promoted, item_pinned)
    }

    #[must_use]
    pub fn open_group(&self, menu: GroupMenuTopic) -> Option<&WidgetGroupIdentity> {
        self.inspection.open_group(menu)
    }

    #[must_use]
    pub fn inspected_target(&self, menu: GroupMenuTopic) -> Option<&HoverTarget> {
        self.inspection.inspected_target(menu)
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
            GroupMenuMessage::RevealMenuGroup {
                menu,
                group_identity,
            } => {
                self.inspection = take(&mut self.inspection).revealing_group(menu, group_identity);
                Task::none()
            }
            GroupMenuMessage::InspectRow { menu, target } => {
                self.inspection = take(&mut self.inspection).inspecting_row(menu, target);
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
        GroupSourceDemand, MENU_SOURCE_LINGER, OpenGroupMenu, WidgetGroupFamily,
        WidgetGroupIdentity,
    };
    use crate::{clock::Clock, widget_groups::widget_group_identity::WidgetGroupInstance};

    const GPU_GROUP: WidgetGroupIdentity = WidgetGroupIdentity::sole(WidgetGroupFamily::Gpu);

    fn gpu_load_item() -> GroupMenuPin {
        GroupMenuPin::Item {
            group: GPU_GROUP,
            item_key: "gpu_load".to_owned(),
        }
    }

    fn toggle_in_hardware(entry: GroupMenuPin) -> GroupMenuMessage {
        GroupMenuMessage::TogglePin {
            menu: GroupMenuTopic::Hardware,
            entry,
        }
    }

    fn hardware_menu_open() -> OpenGroupMenu {
        OpenGroupMenu {
            topic: GroupMenuTopic::Hardware,
            families: vec![WidgetGroupFamily::Gpu],
        }
    }

    fn gpu_demand(group_menus: &GroupMenuState) -> GroupSourceDemand {
        group_menus.source_demand(WidgetGroupFamily::Gpu)
    }

    fn collapsed_on_panel() -> GroupSourceDemand {
        GroupSourceDemand::on_panel()
    }

    fn player_on_bus(player: &'static str, bus: &'static str) -> WidgetGroupIdentity {
        WidgetGroupIdentity {
            family: WidgetGroupFamily::Media,
            instance: WidgetGroupInstance::Session {
                durable: player.into(),
                session: bus.into(),
            },
        }
    }

    #[test]
    fn toggling_twice_unpins_the_group() {
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(toggle_in_hardware(GroupMenuPin::Group(GPU_GROUP)));
        assert!(group_menus.is_pinned(GroupMenuTopic::Hardware, GroupMenuPin::Group(GPU_GROUP)));
        assert!(!group_menus.is_pinned(GroupMenuTopic::Behaviour, GroupMenuPin::Group(GPU_GROUP)));
        let _ = group_menus.update(toggle_in_hardware(GroupMenuPin::Group(GPU_GROUP)));
        assert!(!group_menus.is_pinned(GroupMenuTopic::Hardware, GroupMenuPin::Group(GPU_GROUP)));
    }

    #[test]
    fn pinning_an_item_leaves_its_group_unpinned() {
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(toggle_in_hardware(gpu_load_item()));
        assert!(group_menus.is_pinned(GroupMenuTopic::Hardware, gpu_load_item()));
        assert!(!group_menus.is_pinned(GroupMenuTopic::Hardware, GroupMenuPin::Group(GPU_GROUP)));
    }

    #[test]
    fn unpinning_an_expanded_group_drops_its_demand() {
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(toggle_in_hardware(GroupMenuPin::Group(GPU_GROUP)));
        assert_eq!(gpu_demand(&group_menus), collapsed_on_panel());
        let _ = group_menus.update(toggle_in_hardware(GroupMenuPin::Group(GPU_GROUP)));
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
                group: GPU_GROUP,
            }],
            Clock::real().instant,
        );
        assert_eq!(gpu_demand(&group_menus), collapsed_on_panel());
        group_menus.track_source_demand(None, Vec::new(), Clock::real().instant);
        assert_ne!(gpu_demand(&group_menus), collapsed_on_panel());
    }

    #[test]
    fn pinning_one_sibling_item_leaves_the_other() {
        let sibling_item = |item_key: &str| GroupMenuPin::Item {
            group: GPU_GROUP,
            item_key: item_key.to_owned(),
        };
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(toggle_in_hardware(sibling_item("Vpn(Wireguard(\"wg0\"))")));
        assert!(group_menus.is_pinned(
            GroupMenuTopic::Hardware,
            sibling_item("Vpn(Wireguard(\"wg0\"))")
        ));
        assert!(!group_menus.is_pinned(
            GroupMenuTopic::Hardware,
            sibling_item("Vpn(Wireguard(\"wg1\"))")
        ));
    }

    #[test]
    fn pinned_item_demand_is_limited_to_its_own_family() {
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(toggle_in_hardware(gpu_load_item()));
        assert_ne!(
            gpu_demand(&group_menus),
            group_menus.source_demand(WidgetGroupFamily::Compute)
        );
    }

    #[test]
    fn a_pinned_instance_demands_the_source_its_family_shares() {
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(toggle_in_hardware(GroupMenuPin::Group(
            WidgetGroupIdentity::durable(WidgetGroupFamily::EthernetPorts, "enp3s0"),
        )));
        assert_eq!(
            group_menus.source_demand(WidgetGroupFamily::EthernetPorts),
            collapsed_on_panel()
        );
        assert_ne!(
            group_menus.source_demand(WidgetGroupFamily::WifiAdapters),
            collapsed_on_panel()
        );
    }

    #[test]
    fn demand_outlasts_the_open_menu_for_the_linger_only() {
        let mut group_menus = GroupMenuState::default();
        let opened_at = Clock::real().instant;
        group_menus.track_source_demand(Some(hardware_menu_open()), Vec::new(), opened_at);
        assert_ne!(gpu_demand(&group_menus), collapsed_on_panel());
        group_menus.track_source_demand(None, Vec::new(), opened_at + Duration::from_secs(1));
        assert_ne!(
            gpu_demand(&group_menus),
            GroupMenuState::default().source_demand(WidgetGroupFamily::Gpu)
        );
        group_menus.track_source_demand(None, Vec::new(), opened_at + MENU_SOURCE_LINGER);
        assert_eq!(
            gpu_demand(&group_menus),
            GroupMenuState::default().source_demand(WidgetGroupFamily::Gpu)
        );
    }

    #[test]
    fn a_pin_follows_its_source_not_its_position() {
        let first_port = WidgetGroupIdentity::durable(WidgetGroupFamily::EthernetPorts, "enp3s0");
        let second_port = WidgetGroupIdentity::durable(WidgetGroupFamily::EthernetPorts, "enp4s0");
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(toggle_in_hardware(GroupMenuPin::Group(second_port.clone())));
        assert!(group_menus.is_pinned(GroupMenuTopic::Hardware, GroupMenuPin::Group(second_port)));
        assert!(!group_menus.is_pinned(GroupMenuTopic::Hardware, GroupMenuPin::Group(first_port)));
    }

    #[test]
    fn namesake_players_share_a_pin() {
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(toggle_in_hardware(GroupMenuPin::Group(player_on_bus(
            "Firefox", ":1.40",
        ))));
        assert!(group_menus.is_pinned(
            GroupMenuTopic::Hardware,
            GroupMenuPin::Group(player_on_bus("Firefox", ":1.41"))
        ));
        assert!(!group_menus.is_pinned(
            GroupMenuTopic::Hardware,
            GroupMenuPin::Group(player_on_bus("Spotify", ":1.42"))
        ));
    }

    #[test]
    fn closing_forgets_the_open_group() {
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(GroupMenuMessage::RevealMenuGroup {
            menu: GroupMenuTopic::Hardware,
            group_identity: GPU_GROUP,
        });
        assert_eq!(
            group_menus.open_group(GroupMenuTopic::Hardware),
            Some(&GPU_GROUP)
        );
        let _ = group_menus.update(GroupMenuMessage::Closed);
        assert_eq!(group_menus.open_group(GroupMenuTopic::Hardware), None);
    }

    #[test]
    fn an_open_group_survives_pinning_an_earlier_group() {
        let compute = WidgetGroupIdentity::sole(WidgetGroupFamily::Compute);
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(GroupMenuMessage::RevealMenuGroup {
            menu: GroupMenuTopic::Hardware,
            group_identity: GPU_GROUP,
        });
        let _ = group_menus.update(toggle_in_hardware(GroupMenuPin::Group(compute)));
        assert_eq!(
            group_menus.open_group(GroupMenuTopic::Hardware),
            Some(&GPU_GROUP)
        );
    }
}
