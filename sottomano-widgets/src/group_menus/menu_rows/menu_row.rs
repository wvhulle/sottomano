use std::borrow::Cow;

use corredo::popovers::{NestedMenuMetrics, NestedMenuRowContent, NestedMenuRowEmphasis};
use iced::{Element, widget::text::Wrapping};
use sottomano_toml_config::Config;

use crate::{
    group_items::group_item::MenuParts,
    widget_groups::widget_group_content::{GroupEvent, RowInspection},
    widgets::text::panel_text,
};

pub(crate) struct MenuRowContent<'a, M> {
    pub leading: Vec<Element<'a, GroupEvent<M>>>,
    pub parts: MenuParts<'a, M>,
    pub name: Cow<'static, str>,
    pub trailing: Vec<Element<'a, GroupEvent<M>>>,
}

impl From<RowInspection> for NestedMenuRowEmphasis {
    fn from(row_inspection: RowInspection) -> Self {
        match row_inspection {
            RowInspection::Inspected => NestedMenuRowEmphasis::Selected,
            RowInspection::Uninspected => NestedMenuRowEmphasis::Plain,
        }
    }
}

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

pub(crate) fn menu_row_content<'a, M: From<GroupEvent<M>> + 'a>(
    config: &Config,
    content: MenuRowContent<'a, M>,
) -> NestedMenuRowContent<'a, M> {
    let MenuRowContent {
        leading,
        parts,
        name,
        trailing,
    } = content;
    let lifted = |element: Element<'a, GroupEvent<M>>| element.map(M::from);
    NestedMenuRowContent {
        leading: leading.into_iter().map(lifted).collect(),
        icon: lifted(parts.icon),
        label: Element::from(panel_text(&config.theme.font, name).wrapping(Wrapping::None))
            .map(M::from),
        value: lifted(parts.value),
        trailing: trailing.into_iter().map(lifted).collect(),
    }
}
