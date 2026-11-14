use std::borrow::Cow;

use corredo::popovers::{
    NestedMenuMetrics, NestedMenuRowContent, NestedMenuRowEmphasis, NestedMenuRowLook,
    NestedMenuRowPress, RowHoverReaction, nested_menu_row,
};
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

pub(crate) fn menu_row<'a, Message: Clone + 'a>(
    config: &'a Config,
    content: MenuRowContent<'a, Message>,
    look: NestedMenuRowLook,
    press: NestedMenuRowPress<GroupEvent<Message>>,
    hover_reaction: RowHoverReaction<GroupEvent<Message>>,
) -> Element<'a, GroupEvent<Message>> {
    let MenuRowContent {
        leading,
        parts,
        name,
        trailing,
    } = content;
    nested_menu_row(
        menu_metrics(config),
        NestedMenuRowContent {
            leading,
            icon: parts.icon,
            label: panel_text(&config.theme.font, name)
                .wrapping(Wrapping::None)
                .into(),
            value: parts.value,
            trailing,
        },
        look,
        press,
        hover_reaction,
    )
}
