use corredo::popovers::NestedMenuInspection;

use crate::{
    group_menus::group_menu_state::GroupMenuTopic, hover_cards::IndicatorKey,
    widget_groups::widget_group_identity::WidgetGroupIdentity,
};

pub(crate) type GroupMenuInspection =
    NestedMenuInspection<GroupMenuTopic, WidgetGroupIdentity, IndicatorKey>;
