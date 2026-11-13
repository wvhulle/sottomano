use std::iter::once;

use iced::{
    Element,
    border::Radius,
    widget::{Space, mouse_area},
};
use sottomano_toml_config::Config;

use crate::{
    group_items::{group_item::MenuParts, group_item_gestures::GroupItemGesture},
    group_menus::{
        cascading_layout::{
            cascade_row::CascadeDirection, cascade_satellite_panels::CascadeSatellitePanels,
            notched_panel::PanelGrowth,
        },
        group_menu_presentation::{GroupMenuLayout, GroupMenuPresentation},
        group_menu_state::{GroupMenuPin, GroupMenuState},
        group_menu_view::GroupMenuMessenger,
        menu_rows::{
            menu_row::{MenuRowContent, MenuRowEmphasis, MenuRowLook, MenuRowPress, menu_row},
            pointer_tracked_row::{RowHoverReaction, SatelliteReach},
        },
    },
    hover_cards::{HoverTarget, marked_by_hover_target, marked_by_menu_pin},
    widget_groups::{
        widget_group_content::{GroupEvent, GroupItemPin, GroupMenuItemRow},
        widget_group_identity::{WidgetGroupIdentity, WidgetGroupTitle},
    },
    widgets::{
        configured_theme::{secondary_text_color, text_color},
        glyph_button,
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        uniform_width_column::uniform_width_column,
    },
};

pub(crate) struct GroupMenuGroupRow<'a, M> {
    pub group_identity: WidgetGroupIdentity,
    pub group_title: WidgetGroupTitle,
    pub face: MenuParts<'a, M>,
    pub items: Vec<GroupMenuItemRow<'a, M>>,
}

pub(crate) fn one_row_per_identity<'a, M>(
    rows: impl IntoIterator<Item = GroupMenuGroupRow<'a, M>>,
) -> Vec<GroupMenuGroupRow<'a, M>> {
    rows.into_iter().fold(Vec::new(), |mut merged, row| {
        match merged
            .iter_mut()
            .find(|existing: &&mut GroupMenuGroupRow<'a, M>| {
                existing.group_identity == row.group_identity
            }) {
            Some(existing) => existing.items.extend(row.items),
            None => merged.push(row),
        }
        merged
    })
}

pub(crate) struct GroupMenuRowContext<'p, 'a, M> {
    pub config: &'a Config,
    pub direction: CascadeDirection,
    pub presentation: GroupMenuPresentation,
    pub menu: &'p GroupMenuMessenger<M>,
    pub group_menus: &'p GroupMenuState,
    pub cascade_satellite_panels: &'p CascadeSatellitePanels,
}

const fn accordion_caret(emphasis: MenuRowEmphasis, growth: PanelGrowth) -> Phosphor {
    match (emphasis, growth) {
        (MenuRowEmphasis::Plain, _) => Phosphor::CaretRight(Weight::Regular),
        (MenuRowEmphasis::Selected, PanelGrowth::Downward) => Phosphor::CaretDown(Weight::Regular),
        (MenuRowEmphasis::Selected, PanelGrowth::Upward) => Phosphor::CaretUp(Weight::Regular),
    }
}

pub(crate) fn group_row<'a, M: Clone + 'a>(
    layout: &GroupMenuRowContext<'_, 'a, M>,
    entry: GroupMenuGroupRow<'a, M>,
    look: MenuRowLook,
) -> Element<'a, GroupEvent<M>> {
    let icon_size = layout.config.theme.font.icon_size_px;
    let group_title = entry.group_title;
    let pinned_entry = GroupMenuPin::Group(entry.group_identity.clone());
    let pin = marked_by_menu_pin(
        glyph_button(
            Phosphor::PushPin(Weight::Regular),
            icon_size,
            secondary_text_color,
            layout.menu.toggle_pin(pinned_entry.clone()),
        ),
        pinned_entry,
    );
    let caret_color = match look.emphasis {
        MenuRowEmphasis::Plain => secondary_text_color,
        MenuRowEmphasis::Selected => text_color,
    };
    let (leading, trailing) = match (layout.presentation.layout, layout.direction) {
        (GroupMenuLayout::Accordion, _) => (
            Vec::new(),
            vec![
                pin,
                IconGlyph::from(accordion_caret(
                    look.emphasis,
                    layout.cascade_satellite_panels.growth,
                ))
                .view(icon_size, caret_color),
            ],
        ),
        (GroupMenuLayout::Cascading, CascadeDirection::Rightward) => (
            Vec::new(),
            vec![
                pin,
                IconGlyph::from(Phosphor::CaretRight(Weight::Regular)).view(icon_size, caret_color),
            ],
        ),
        (GroupMenuLayout::Cascading, CascadeDirection::Leftward) => (
            vec![
                IconGlyph::from(Phosphor::CaretLeft(Weight::Regular)).view(icon_size, caret_color),
            ],
            vec![pin],
        ),
    };
    let content = MenuRowContent {
        leading,
        parts: entry.face,
        name: group_title.to_string().into(),
        trailing,
    };
    let group_identity = entry.group_identity;
    let (press_event, hover_reaction) = match layout.presentation.layout {
        GroupMenuLayout::Cascading => (
            layout.menu.reveal_group(group_identity.clone()),
            RowHoverReaction::Reveals {
                reveal: layout.menu.reveal_group(group_identity.clone()),
                defer_reveal: layout.menu.intend_group_reveal(group_identity.clone()),
                cancel_reveal: layout.menu.cancel_group_reveal(group_identity),
                satellite_reach: if layout.group_menus.open_groups(layout.menu.topic).is_empty() {
                    SatelliteReach::Closed
                } else {
                    SatelliteReach::Open(layout.direction)
                },
            },
        ),
        GroupMenuLayout::Accordion => (
            layout.menu.toggle_group(group_identity),
            RowHoverReaction::Highlights,
        ),
    };
    menu_row(
        layout.config,
        content,
        look,
        MenuRowPress::Performs(press_event),
        hover_reaction,
    )
}

pub(crate) fn item_menu<'a, M: Clone + From<GroupEvent<M>> + 'a>(
    layout: &GroupMenuRowContext<'_, 'a, M>,
    group: &WidgetGroupIdentity,
    items: Vec<GroupMenuItemRow<'a, M>>,
    inspected_target: Option<&HoverTarget>,
    panel_corners: Radius,
) -> Element<'a, M> {
    let rows: Vec<Element<'a, M>> = items
        .into_iter()
        .map(|item| {
            let look = MenuRowLook {
                emphasis: MenuRowEmphasis::from(item.inspection(inspected_target)),
                frame_corners: panel_corners,
            };
            item_row(layout, group, item, look).map(M::from)
        })
        .collect();
    uniform_width_column(rows).into()
}

fn card_info_icon<'a, M: Clone + 'a>(
    layout: &GroupMenuRowContext<'_, 'a, M>,
    target: HoverTarget,
) -> Element<'a, GroupEvent<M>> {
    let inspect = layout.menu.inspect_row(target.clone());
    let icon = glyph_button(
        Phosphor::Info(Weight::Regular),
        layout.config.theme.font.icon_size_px,
        secondary_text_color,
        inspect.clone(),
    );
    marked_by_hover_target(mouse_area(icon).on_enter(inspect).into(), target)
}

pub(crate) fn item_row<'a, M: Clone + 'a>(
    layout: &GroupMenuRowContext<'_, 'a, M>,
    group: &WidgetGroupIdentity,
    item: GroupMenuItemRow<'a, M>,
    look: MenuRowLook,
) -> Element<'a, GroupEvent<M>> {
    let pin_button = |pin_weight, pin_color: fn(&iced::Theme) -> iced::Color, item_key| {
        glyph_button(
            Phosphor::PushPin(pin_weight),
            layout.config.theme.font.icon_size_px,
            pin_color,
            layout.menu.toggle_pin(GroupMenuPin::Item {
                group: group.clone(),
                item_key,
            }),
        )
    };
    let item_pin = match item.pin {
        GroupItemPin::Unpinnable => None,
        GroupItemPin::Unpinned(item_key) => {
            Some(pin_button(Weight::Regular, secondary_text_color, item_key))
        }
        GroupItemPin::Pinned(item_key) => Some(pin_button(Weight::Fill, text_color, item_key)),
    };
    let info_icon = card_info_icon(layout, item.inspected_target);
    let (leading, trailing) = match (layout.presentation.layout, layout.direction) {
        (GroupMenuLayout::Accordion, _) => (
            vec![
                Space::new()
                    .width(layout.config.theme.font.icon_size_px)
                    .into(),
            ],
            item_pin.into_iter().chain(once(info_icon)).collect(),
        ),
        (GroupMenuLayout::Cascading, CascadeDirection::Rightward) => (
            Vec::new(),
            item_pin.into_iter().chain(once(info_icon)).collect(),
        ),
        (GroupMenuLayout::Cascading, CascadeDirection::Leftward) => {
            (vec![info_icon], item_pin.into_iter().collect())
        }
    };
    let content = MenuRowContent {
        leading,
        parts: item.parts,
        name: item.name,
        trailing,
    };
    let press = match item.gestures.left {
        GroupItemGesture::Performs { message, .. } => {
            MenuRowPress::Performs(GroupEvent::Group(message))
        }
        GroupItemGesture::Inert => MenuRowPress::Inert,
    };
    let row = menu_row(
        layout.config,
        content,
        look,
        press,
        RowHoverReaction::Highlights,
    );
    match item.gestures.right {
        GroupItemGesture::Performs { message, .. } => mouse_area(row)
            .on_right_press(GroupEvent::Group(message))
            .into(),
        GroupItemGesture::Inert => row,
    }
}

#[cfg(test)]
mod tests {
    use iced::widget::Space;

    use super::{GroupMenuGroupRow, WidgetGroupIdentity, one_row_per_identity};
    use crate::{
        group_items::group_item::MenuParts,
        widget_groups::widget_group_identity::{WidgetGroupFamily, WidgetGroupTitle},
    };

    fn row_of(group_identity: WidgetGroupIdentity, title: &str) -> GroupMenuGroupRow<'static, ()> {
        GroupMenuGroupRow {
            group_identity,
            group_title: WidgetGroupTitle::Named(title.to_owned().into()),
            face: MenuParts {
                icon: Space::new().into(),
                value: Space::new().into(),
            },
            items: Vec::new(),
        }
    }

    #[test]
    fn groups_of_one_identity_share_a_row() {
        let gpu = WidgetGroupIdentity::sole(WidgetGroupFamily::Gpu);
        let rows = one_row_per_identity([row_of(gpu.clone(), "GPU"), row_of(gpu, "GPU")]);
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn instances_of_one_family_keep_their_own_rows() {
        let rows = one_row_per_identity([
            row_of(
                WidgetGroupIdentity::durable(WidgetGroupFamily::PhysicalDrive, "sda"),
                "a",
            ),
            row_of(
                WidgetGroupIdentity::durable(WidgetGroupFamily::PhysicalDrive, "sdb"),
                "b",
            ),
        ]);
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn a_player_named_audio_keeps_its_own_row() {
        let rows = one_row_per_identity([
            row_of(WidgetGroupIdentity::sole(WidgetGroupFamily::Audio), "Audio"),
            row_of(
                WidgetGroupIdentity::durable(WidgetGroupFamily::Media, "Audio"),
                "Audio",
            ),
        ]);
        assert_eq!(rows.len(), 2);
    }
}
