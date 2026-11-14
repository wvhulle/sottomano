use corredo::popovers::{
    NestedMenuLayout, NestedMenuOpenTrigger, OpenGroupLimit, SubmenuConnector,
};

use crate::group_items::toggle_control::SwitchPosition;

impl From<SubmenuConnector> for SwitchPosition {
    fn from(submenu_connector: SubmenuConnector) -> Self {
        match submenu_connector {
            SubmenuConnector::Shown => SwitchPosition::On,
            SubmenuConnector::Hidden => SwitchPosition::Off,
        }
    }
}

impl From<NestedMenuLayout> for SwitchPosition {
    fn from(layout: NestedMenuLayout) -> Self {
        match layout {
            NestedMenuLayout::Accordion => SwitchPosition::On,
            NestedMenuLayout::Cascading => SwitchPosition::Off,
        }
    }
}

impl From<NestedMenuOpenTrigger> for SwitchPosition {
    fn from(open_trigger: NestedMenuOpenTrigger) -> Self {
        match open_trigger {
            NestedMenuOpenTrigger::Press => SwitchPosition::On,
            NestedMenuOpenTrigger::Hover => SwitchPosition::Off,
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
