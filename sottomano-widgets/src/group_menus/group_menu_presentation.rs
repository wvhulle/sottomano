use crate::group_items::toggle_control::SwitchPosition;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GroupMenuLayout {
    #[default]
    Cascading,
    Accordion,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GroupMenuOpenTrigger {
    Hover,
    #[default]
    Press,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OpenGroupLimit {
    One,
    #[default]
    Several,
}

impl From<GroupMenuLayout> for SwitchPosition {
    fn from(layout: GroupMenuLayout) -> Self {
        match layout {
            GroupMenuLayout::Accordion => SwitchPosition::On,
            GroupMenuLayout::Cascading => SwitchPosition::Off,
        }
    }
}

impl From<GroupMenuOpenTrigger> for SwitchPosition {
    fn from(open_trigger: GroupMenuOpenTrigger) -> Self {
        match open_trigger {
            GroupMenuOpenTrigger::Press => SwitchPosition::On,
            GroupMenuOpenTrigger::Hover => SwitchPosition::Off,
        }
    }
}

impl From<OpenGroupLimit> for SwitchPosition {
    fn from(open_group_limit: OpenGroupLimit) -> Self {
        match open_group_limit {
            OpenGroupLimit::One => SwitchPosition::On,
            OpenGroupLimit::Several => SwitchPosition::Off,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GroupMenuPresentation {
    pub layout: GroupMenuLayout,
    pub open_trigger: GroupMenuOpenTrigger,
    pub accordion_open_limit: OpenGroupLimit,
}

impl GroupMenuPresentation {
    #[must_use]
    pub const fn reveals_on_hover(self) -> bool {
        matches!(self.layout, GroupMenuLayout::Cascading)
    }

    #[must_use]
    pub const fn open_group_limit(self) -> OpenGroupLimit {
        match self.layout {
            GroupMenuLayout::Cascading => OpenGroupLimit::One,
            GroupMenuLayout::Accordion => self.accordion_open_limit,
        }
    }

    #[must_use]
    pub const fn with_layout_toggled(self) -> Self {
        Self {
            layout: match self.layout {
                GroupMenuLayout::Cascading => GroupMenuLayout::Accordion,
                GroupMenuLayout::Accordion => GroupMenuLayout::Cascading,
            },
            ..self
        }
    }

    #[must_use]
    pub const fn with_open_trigger_toggled(self) -> Self {
        Self {
            open_trigger: match self.open_trigger {
                GroupMenuOpenTrigger::Hover => GroupMenuOpenTrigger::Press,
                GroupMenuOpenTrigger::Press => GroupMenuOpenTrigger::Hover,
            },
            ..self
        }
    }

    #[must_use]
    pub const fn with_open_limit_toggled(self) -> Self {
        Self {
            accordion_open_limit: match self.accordion_open_limit {
                OpenGroupLimit::One => OpenGroupLimit::Several,
                OpenGroupLimit::Several => OpenGroupLimit::One,
            },
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GroupMenuLayout, GroupMenuOpenTrigger, GroupMenuPresentation, OpenGroupLimit};

    #[test]
    fn only_a_cascading_menu_reveals_on_hover_whatever_the_open_trigger() {
        for open_trigger in [GroupMenuOpenTrigger::Hover, GroupMenuOpenTrigger::Press] {
            let cascading = GroupMenuPresentation {
                open_trigger,
                ..GroupMenuPresentation::default()
            };
            assert!(cascading.reveals_on_hover());
            let accordion = GroupMenuPresentation {
                layout: GroupMenuLayout::Accordion,
                open_trigger,
                ..GroupMenuPresentation::default()
            };
            assert!(!accordion.reveals_on_hover());
        }
    }

    #[test]
    fn menus_open_on_press_until_a_stored_choice_says_otherwise() {
        assert_eq!(
            GroupMenuPresentation::default().open_trigger,
            GroupMenuOpenTrigger::Press
        );
    }

    #[test]
    fn switching_layouts_keeps_each_sub_choice() {
        let chosen = GroupMenuPresentation::default()
            .with_open_trigger_toggled()
            .with_open_limit_toggled();
        let round_trip = chosen.with_layout_toggled().with_layout_toggled();
        assert_eq!(round_trip, chosen);
        assert_eq!(chosen.open_trigger, GroupMenuOpenTrigger::Hover);
        assert_eq!(chosen.accordion_open_limit, OpenGroupLimit::One);
    }

    #[test]
    fn a_cascading_menu_opens_one_group_whatever_the_accordion_limit() {
        let several_stored = GroupMenuPresentation {
            accordion_open_limit: OpenGroupLimit::Several,
            ..GroupMenuPresentation::default()
        };
        assert_eq!(several_stored.open_group_limit(), OpenGroupLimit::One);
    }
}
