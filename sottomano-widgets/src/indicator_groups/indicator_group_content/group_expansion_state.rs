use std::collections::{BTreeMap, BTreeSet};

use corredo::transitions::ExpansionState;
use iced::mouse::ScrollDelta;

use crate::hover_cards::{MenuHandleIndicator, PointerPresence};

#[derive(Debug, Clone, Copy)]
pub enum IndicatorGroupExpansionMessage {
    Toggle,
    Expand,
    CursorEntered,
    CursorExited,
    Collapse,
    HandleScrolled(ScrollDelta),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpansionChange {
    JustCollapsed,
    Unchanged,
}

#[derive(Default)]
pub struct IndicatorGroupExpansion {
    pub state: ExpansionState,
    cursor_pointing: PointerPresence,
}

pub struct KeyedExpansions<Key> {
    pub expansions: BTreeMap<Key, IndicatorGroupExpansion>,
}

impl<Key> Default for KeyedExpansions<Key> {
    fn default() -> Self {
        Self {
            expansions: BTreeMap::new(),
        }
    }
}

impl<Key: Ord> KeyedExpansions<Key> {
    pub fn collapse(&mut self) {
        self.expansions.values_mut().for_each(|expansion| {
            expansion.collapse();
        });
    }

    #[must_use]
    pub fn combined_state(&self) -> ExpansionState {
        self.expansions
            .values()
            .map(|expansion| expansion.state)
            .find(|expansion_state| *expansion_state == ExpansionState::Expanded)
            .unwrap_or(ExpansionState::Collapsed)
    }

    pub fn update(&mut self, key: &Key, message: IndicatorGroupExpansionMessage) {
        if let Some(expansion) = self.expansions.get_mut(key) {
            expansion.update(message);
        }
    }

    pub fn clear(&mut self) {
        self.expansions.clear();
    }

    pub fn track(&mut self, keys: impl IntoIterator<Item = Key>) {
        let current: BTreeSet<Key> = keys.into_iter().collect();
        self.expansions.retain(|key, _| current.contains(key));
        for key in current {
            self.expansions.entry(key).or_default();
        }
    }
}

impl IndicatorGroupExpansion {
    #[must_use]
    pub fn handle_indicator(&self) -> MenuHandleIndicator {
        MenuHandleIndicator {
            pointing: self.cursor_pointing,
            menu_expansion: self.state,
        }
    }

    pub fn update(&mut self, msg: IndicatorGroupExpansionMessage) -> ExpansionChange {
        let previous_state = self.state;
        match msg {
            IndicatorGroupExpansionMessage::Toggle => {
                self.state = match self.state {
                    ExpansionState::Expanded => ExpansionState::Collapsed,
                    ExpansionState::Collapsed => ExpansionState::Expanded,
                }
            }
            IndicatorGroupExpansionMessage::Expand => self.state = ExpansionState::Expanded,
            IndicatorGroupExpansionMessage::CursorEntered => {
                self.cursor_pointing = PointerPresence::PointedAt;
            }
            IndicatorGroupExpansionMessage::CursorExited => {
                self.cursor_pointing = PointerPresence::NotPointedAt;
            }
            IndicatorGroupExpansionMessage::Collapse => self.state = ExpansionState::Collapsed,
            IndicatorGroupExpansionMessage::HandleScrolled(_) => {}
        }
        match (previous_state, self.state) {
            (ExpansionState::Expanded, ExpansionState::Collapsed) => ExpansionChange::JustCollapsed,
            _ => ExpansionChange::Unchanged,
        }
    }

    pub fn collapse(&mut self) -> ExpansionChange {
        self.update(IndicatorGroupExpansionMessage::Collapse)
    }
}
