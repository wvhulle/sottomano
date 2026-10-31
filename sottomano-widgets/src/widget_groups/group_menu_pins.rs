mod sqlite_group_menu_pins;

use std::collections::BTreeSet;

use iced::Task;

use self::sqlite_group_menu_pins::StoredGroupMenuPin;
use crate::hover_tooltips::GroupHandle;

#[derive(Debug, Clone)]
pub enum GroupMenuPinsMessage {
    Loaded(BTreeSet<StoredGroupMenuPin>),
    Toggle {
        group_menu: GroupHandle,
        group_title: &'static str,
    },
    Persisted(bool),
}

#[derive(Default)]
pub struct GroupMenuPins {
    pinned: BTreeSet<StoredGroupMenuPin>,
}

impl GroupMenuPins {
    pub fn load() -> Task<GroupMenuPinsMessage> {
        Task::perform(sqlite_group_menu_pins::load(), GroupMenuPinsMessage::Loaded)
    }

    #[must_use]
    pub fn is_pinned(&self, group_menu: GroupHandle, group_title: &str) -> bool {
        self.pinned.contains(&stored_pin(group_menu, group_title))
    }

    pub fn update(&mut self, message: GroupMenuPinsMessage) -> Task<GroupMenuPinsMessage> {
        match message {
            GroupMenuPinsMessage::Loaded(pinned) => {
                self.pinned = pinned;
                Task::none()
            }
            GroupMenuPinsMessage::Toggle {
                group_menu,
                group_title,
            } => {
                let pin = stored_pin(group_menu, group_title);
                let pinned = !self.pinned.remove(&pin);
                if pinned {
                    self.pinned.insert(pin.clone());
                }
                Task::perform(
                    sqlite_group_menu_pins::save(pin, pinned),
                    GroupMenuPinsMessage::Persisted,
                )
            }
            GroupMenuPinsMessage::Persisted(saved) => {
                if !saved {
                    tracing::warn!("group menu pins: the pin change was not saved");
                }
                Task::none()
            }
        }
    }
}

fn stored_pin(group_menu: GroupHandle, group_title: &str) -> StoredGroupMenuPin {
    StoredGroupMenuPin {
        group_menu: format!("{group_menu:?}"),
        group_title: group_title.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{GroupMenuPins, GroupMenuPinsMessage};
    use crate::hover_tooltips::GroupHandle;

    #[test]
    fn toggling_twice_unpins_the_group() {
        let mut pins = GroupMenuPins::default();
        let toggle_gpu = || GroupMenuPinsMessage::Toggle {
            group_menu: GroupHandle::HardwareMenu,
            group_title: "GPU",
        };
        let _ = pins.update(toggle_gpu());
        assert!(pins.is_pinned(GroupHandle::HardwareMenu, "GPU"));
        assert!(!pins.is_pinned(GroupHandle::BehaviourMenu, "GPU"));
        let _ = pins.update(toggle_gpu());
        assert!(!pins.is_pinned(GroupHandle::HardwareMenu, "GPU"));
    }
}
