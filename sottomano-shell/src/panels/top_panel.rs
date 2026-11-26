use sottomano_toml_config::screen_edges::Position;
use sottomano_widgets::group_menus::group_menu_state::GroupMenuTopic;

use crate::panels::{PanelBarComposition, PanelZoneEntry::Menu};

pub(crate) const TOP_PANEL: PanelBarComposition = PanelBarComposition {
    position: Position::Top,
    start: &[
        Menu(GroupMenuTopic::Hardware),
        Menu(GroupMenuTopic::Network),
        Menu(GroupMenuTopic::WindowMenuBar),
    ],
    center: &[Menu(GroupMenuTopic::Agenda)],
    end: &[
        Menu(GroupMenuTopic::Display),
        Menu(GroupMenuTopic::Media),
        Menu(GroupMenuTopic::Gaming),
        Menu(GroupMenuTopic::SessionPower),
    ],
};
