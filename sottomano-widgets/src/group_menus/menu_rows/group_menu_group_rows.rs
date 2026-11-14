use corredo::{
    popovers::{CascadeDirection, PopupGrowth},
    theming::{primary_weak_text_color, secondary_text_color, text_color},
    width_layout::equal_width_column,
};
use iced::{
    Element,
    border::Radius,
    widget::{Space, mouse_area},
};
use sottomano_toml_config::Config;

use crate::{
    group_items::{
        group_item::{MenuParts, RowScroll},
        group_item_gestures::GroupItemGesture,
    },
    group_menus::{
        cascading_layout::cascade_satellite_panels::CascadeSatellitePanels,
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
        glyph_button,
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
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

const fn accordion_caret(emphasis: MenuRowEmphasis, growth: PopupGrowth) -> Phosphor {
    match (emphasis, growth) {
        (MenuRowEmphasis::Plain, _) => Phosphor::CaretRight(Weight::Regular),
        (MenuRowEmphasis::Selected, PopupGrowth::Downward) => Phosphor::CaretDown(Weight::Regular),
        (MenuRowEmphasis::Selected, PopupGrowth::Upward) => Phosphor::CaretUp(Weight::Regular),
    }
}

fn trailing_glyph_color(emphasis: MenuRowEmphasis) -> fn(&iced::Theme) -> iced::Color {
    match emphasis {
        MenuRowEmphasis::Plain => secondary_text_color,
        MenuRowEmphasis::Selected => primary_weak_text_color,
    }
}

fn pinned_glyph_color(emphasis: MenuRowEmphasis) -> fn(&iced::Theme) -> iced::Color {
    match emphasis {
        MenuRowEmphasis::Plain => text_color,
        MenuRowEmphasis::Selected => primary_weak_text_color,
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
            trailing_glyph_color(look.emphasis),
            layout.menu.toggle_pin(pinned_entry.clone()),
        ),
        pinned_entry,
    );
    let caret_color = trailing_glyph_color(look.emphasis);
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
    equal_width_column(rows).into()
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
        GroupItemPin::Unpinned(item_key) => Some(pin_button(
            Weight::Regular,
            trailing_glyph_color(look.emphasis),
            item_key,
        )),
        GroupItemPin::Pinned(item_key) => Some(pin_button(
            Weight::Fill,
            pinned_glyph_color(look.emphasis),
            item_key,
        )),
    };
    let inspected_target = item.inspected_target;
    let (leading, trailing) = match (layout.presentation.layout, layout.direction) {
        (GroupMenuLayout::Accordion, _) => (
            vec![
                Space::new()
                    .width(layout.config.theme.font.icon_size_px)
                    .into(),
            ],
            item_pin.into_iter().collect(),
        ),
        (GroupMenuLayout::Cascading, _) => (Vec::new(), item_pin.into_iter().collect()),
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
    let inspect = layout.menu.inspect_row(inspected_target.clone());
    let inspected_on_enter = match item.row_scroll {
        RowScroll::AdjustsValue(adjust_by_scroll) => mouse_area(row)
            .on_enter(inspect)
            .on_scroll(move |delta| GroupEvent::Group(adjust_by_scroll(delta))),
        RowScroll::Inert => mouse_area(row).on_enter(inspect),
    };
    let gestured_row = match item.gestures.right {
        GroupItemGesture::Performs { message, .. } => inspected_on_enter
            .on_right_press(GroupEvent::Group(message))
            .into(),
        GroupItemGesture::Inert => inspected_on_enter.into(),
    };
    marked_by_hover_target(gestured_row, inspected_target)
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
