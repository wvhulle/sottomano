use sottomano_toml_config::screen_edges::Position;
use sottomano_widgets::group_menus::group_menu_state::GroupMenuTopic;

use crate::panels::{
    FixedPanelEntry::SessionPower,
    PanelBarComposition,
    PanelZoneEntry::{Fixed, Menu, Standalone},
    StandaloneWidgetGroup::{Clock, Notifications},
};

pub(crate) const TOP_PANEL: PanelBarComposition = PanelBarComposition {
    position: Position::Top,
    start: &[
        Menu(GroupMenuTopic::Hardware),
        Menu(GroupMenuTopic::Network),
        Menu(GroupMenuTopic::ApplicationMenu),
    ],
    center: &[
        Standalone(Clock),
        Menu(GroupMenuTopic::Agenda),
        Standalone(Notifications),
    ],
    end: &[
        Menu(GroupMenuTopic::Display),
        Menu(GroupMenuTopic::Media),
        Fixed(SessionPower),
    ],
};
