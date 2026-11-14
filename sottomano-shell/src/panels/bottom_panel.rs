use sottomano_toml_config::screen_edges::Position;
use sottomano_widgets::group_menus::group_menu_state::GroupMenuTopic;

use crate::panels::{
    PanelBarComposition,
    PanelZoneEntry::{InlineWorkspaces, Menu, OutputWindows, Standalone},
    StandaloneWidgetGroup::Tray,
};

pub(crate) const BOTTOM_PANEL: PanelBarComposition = PanelBarComposition {
    position: Position::Bottom,
    start: &[
        Menu(GroupMenuTopic::Applications),
        Menu(GroupMenuTopic::Appearance),
        Menu(GroupMenuTopic::Behaviour),
        InlineWorkspaces,
    ],
    center: &[OutputWindows],
    end: &[
        Menu(GroupMenuTopic::Virtualization),
        Menu(GroupMenuTopic::Services),
        Menu(GroupMenuTopic::Session),
        Standalone(Tray),
    ],
};
