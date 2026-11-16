use corredo::popovers::NestedMenuInspection;

use crate::{
    group_menus::group_menu_state::GroupMenuTopic, hover_cards::IndicatorKey,
    indicator_groups::indicator_group_identity::IndicatorGroupIdentity,
};

pub(crate) type GroupMenuInspection =
    NestedMenuInspection<GroupMenuTopic, IndicatorGroupIdentity, IndicatorKey>;
