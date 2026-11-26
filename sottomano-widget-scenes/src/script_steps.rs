use std::{borrow::Cow, time::Duration};

use sottomano_widgets::{
    group_indicators::indicator_group_identity::{IndicatorGroupFamily, IndicatorGroupIdentity},
    group_menus::group_menu_state::{GroupMenuPin, PanelPin},
    hover_cards::{HoverTarget, IndicatorKey},
};

pub enum Step {
    Hover(HoverTarget),
    Press(HoverTarget),
    HoverFace(IndicatorGroupIdentity),
    PressFace(IndicatorGroupIdentity),
    PressPin(GroupMenuPin),
    HoverPinned(PanelPin),
    ReleasePinned(PanelPin),
    PressAway,
    DragFace {
        group: IndicatorGroupIdentity,
        by: f32,
    },
    HoverText(&'static str),
    PressText(&'static str),
    RestFor(Duration),
}

#[must_use]
pub const fn instance_row(group: IndicatorGroupIdentity, row_name: &'static str) -> HoverTarget {
    HoverTarget::Indicator(IndicatorKey {
        group,
        name: Cow::Borrowed(row_name),
    })
}

#[must_use]
pub const fn sole_indicator_key(
    family: IndicatorGroupFamily,
    row_name: &'static str,
) -> IndicatorKey {
    IndicatorKey {
        group: IndicatorGroupIdentity::sole(family),
        name: Cow::Borrowed(row_name),
    }
}

#[must_use]
pub const fn indicator_row(family: IndicatorGroupFamily, row_name: &'static str) -> HoverTarget {
    instance_row(IndicatorGroupIdentity::sole(family), row_name)
}
