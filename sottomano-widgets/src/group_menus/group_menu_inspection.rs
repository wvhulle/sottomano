use corredo::popovers::{NestedMenuAction, NestedMenuPresentation, NestedMenuState};

use crate::{
    group_menus::group_menu_state::GroupMenuTopic, hover_cards::IndicatorKey,
    indicator_groups::indicator_group_identity::IndicatorGroupIdentity,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum ShownTopic {
    #[default]
    Nothing,
    Topic(GroupMenuTopic),
}

#[derive(Debug, Clone, Default, PartialEq)]
enum RowInspection {
    #[default]
    Nothing,
    Row(IndicatorKey),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GroupMenuInspection {
    shown_topic: ShownTopic,
    pub(crate) submenus: NestedMenuState<IndicatorGroupIdentity>,
    row: RowInspection,
}

impl GroupMenuInspection {
    fn showing(&mut self, menu: GroupMenuTopic) {
        if self.shown_topic != ShownTopic::Topic(menu) {
            *self = Self {
                shown_topic: ShownTopic::Topic(menu),
                ..Self::default()
            };
        }
    }

    pub(crate) fn perform(
        &mut self,
        menu: GroupMenuTopic,
        action: NestedMenuAction<IndicatorGroupIdentity>,
    ) {
        self.showing(menu);
        match &action {
            NestedMenuAction::Scrolled { .. } => self.row = RowInspection::Nothing,
            NestedMenuAction::Opened(open_submenus) => {
                if open_submenus.as_slice() != self.submenus.open_submenus {
                    self.row = RowInspection::Nothing;
                }
            }
        }
        self.submenus.perform(action);
    }

    pub(crate) fn inspect_row(&mut self, menu: GroupMenuTopic, indicator: IndicatorKey) {
        if self.shown_topic == ShownTopic::Topic(menu) {
            self.row = RowInspection::Row(indicator);
        }
    }

    pub(crate) fn adopt(&mut self, presentation: NestedMenuPresentation) {
        self.submenus.adopt(presentation);
    }

    pub(crate) fn close(&mut self) {
        *self = Self::default();
    }

    #[must_use]
    pub fn open_groups(&self, menu: GroupMenuTopic) -> &[IndicatorGroupIdentity] {
        if self.shown_topic == ShownTopic::Topic(menu) {
            &self.submenus.open_submenus
        } else {
            &[]
        }
    }

    #[must_use]
    pub fn inspected_indicator(&self, menu: GroupMenuTopic) -> Option<&IndicatorKey> {
        if self.shown_topic != ShownTopic::Topic(menu) {
            return None;
        }
        match &self.row {
            RowInspection::Row(indicator) => Some(indicator),
            RowInspection::Nothing => None,
        }
    }
}
