use std::borrow::Cow;

use corredo::popovers::{NestedMenuMetrics, NestedMenuRowContent};
use iced::{Element, widget::text::Wrapping};
use sottomano_toml_config::Config;

use crate::{widget_groups::widget_group_content::GroupEvent, widgets::text::panel_text};

pub(crate) fn menu_metrics(config: &Config) -> NestedMenuMetrics {
    NestedMenuMetrics {
        row_height_px: config.desktop_panels.height + config.theme.spacing.item_gap,
        icon_slot_px: config.desktop_panels.height,
        item_spacing_px: config.theme.spacing.group_gap,
        side_inset_px: config.theme.spacing.surface_inset,
    }
}

pub(crate) fn menu_row_height(config: &Config) -> f32 {
    menu_metrics(config).row_height_px
}

pub(crate) fn menu_row_content<'a>(
    config: &Config,
    leading: Vec<Element<'a, GroupEvent>>,
    icon: Element<'a, GroupEvent>,
    name: Cow<'static, str>,
    value: Element<'a, GroupEvent>,
    trailing: Vec<Element<'a, GroupEvent>>,
) -> NestedMenuRowContent<'a, GroupEvent> {
    let lifted = |element: Element<'a, GroupEvent>| element;
    NestedMenuRowContent {
        leading: leading.into_iter().map(lifted).collect(),
        icon: lifted(icon),
        label: Element::from(panel_text(&config.theme.font, name).wrapping(Wrapping::None)),
        value: lifted(value),
        trailing: trailing.into_iter().map(lifted).collect(),
    }
}
