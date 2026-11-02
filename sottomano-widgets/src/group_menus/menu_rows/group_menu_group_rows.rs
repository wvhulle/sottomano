use iced::{Element, border::Radius, widget::mouse_area};
use sottomano_config::Config;

use crate::{
    group_items::{group_item::MenuParts, group_item_gestures::GroupItemGesture},
    group_menus::{
        cascading_layout::{
            cascade_row::CascadeDirection, cascade_satellite_panels::CascadeSatellitePanels,
            notched_panel::PanelGrowth,
        },
        group_menu_presentation::{GroupMenuLayout, GroupMenuPresentation, HoverCardPlacement},
        group_menu_state::{GroupMenuPin, GroupMenuState},
        group_menu_view::GroupMenuMessenger,
        menu_rows::menu_row::{MenuRowContent, MenuRowLook, hovered_menu_row, pressable_menu_row},
    },
    hover_cards::{marked_by_hover_target, marked_by_menu_pin},
    widget_groups::{
        widget_group_content::{GroupEvent, GroupItemPin, GroupMenuItemRow},
        widget_group_identity::{WidgetGroupIdentity, WidgetGroupTitle},
    },
    widgets::{
        configured_theme::{secondary_text_color, text_color},
        glyph_button,
        icon_glyph::IconGlyph,
        list_end_corners::ListEndCorners,
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

const fn accordion_caret(open: bool, growth: PanelGrowth) -> Phosphor {
    match (open, growth) {
        (false, _) => Phosphor::CaretRight(Weight::Regular),
        (true, PanelGrowth::Downward) => Phosphor::CaretDown(Weight::Regular),
        (true, PanelGrowth::Upward) => Phosphor::CaretUp(Weight::Regular),
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
    let caret_color = if look.highlighted {
        text_color
    } else {
        secondary_text_color
    };
    let content = match (layout.presentation.layout, layout.direction) {
        (GroupMenuLayout::Accordion, _) => MenuRowContent {
            leading: Vec::new(),
            parts: entry.face,
            name: group_title.to_string().into(),
            trailing: vec![
                pin,
                IconGlyph::from(accordion_caret(
                    look.highlighted,
                    layout.cascade_satellite_panels.growth,
                ))
                .view(icon_size, caret_color),
            ],
        },
        (GroupMenuLayout::Cascading, CascadeDirection::Rightward) => MenuRowContent {
            leading: Vec::new(),
            parts: entry.face,
            name: group_title.to_string().into(),
            trailing: vec![
                pin,
                IconGlyph::from(Phosphor::CaretRight(Weight::Regular)).view(icon_size, caret_color),
            ],
        },
        (GroupMenuLayout::Cascading, CascadeDirection::Leftward) => MenuRowContent {
            leading: vec![
                IconGlyph::from(Phosphor::CaretLeft(Weight::Regular)).view(icon_size, caret_color),
            ],
            parts: entry.face,
            name: group_title.to_string().into(),
            trailing: vec![pin],
        },
    };
    if layout.presentation.reveals_on_hover() {
        let reveal_group = layout.menu.reveal_group(entry.group_identity);
        hovered_menu_row(
            layout.config,
            content,
            look,
            reveal_group.clone(),
            reveal_group,
        )
    } else {
        pressable_menu_row(
            layout.config,
            content,
            look,
            layout.menu.toggle_group(entry.group_identity),
        )
    }
}

pub(crate) fn item_menu<'a, M: Clone + From<GroupEvent<M>> + 'a>(
    layout: &GroupMenuRowContext<'_, 'a, M>,
    group: &WidgetGroupIdentity,
    items: Vec<GroupMenuItemRow<'a, M>>,
    is_inspected: &dyn Fn(&GroupMenuItemRow<'a, M>) -> bool,
    panel_corners: Radius,
    mut inline_card: Option<Element<'a, M>>,
) -> Element<'a, M> {
    let growth = layout.cascade_satellite_panels.growth;
    let end_corners = ListEndCorners::new(items.len(), panel_corners);
    let rows: Vec<Element<'a, M>> = items
        .into_iter()
        .enumerate()
        .flat_map(|(position, item)| {
            let inspected = is_inspected(&item);
            let look = MenuRowLook {
                highlighted: inspected,
                corners: end_corners.radius_at(position),
            };
            let row = item_row(layout, group, item, look).map(M::from);
            match (inspected.then(|| inline_card.take()).flatten(), growth) {
                (Some(card_view), PanelGrowth::Downward) => vec![row, card_view],
                (Some(card_view), PanelGrowth::Upward) => vec![card_view, row],
                (None, _) => vec![row],
            }
        })
        .collect();
    uniform_width_column(rows).into()
}

pub(crate) fn item_row<'a, M: Clone + 'a>(
    layout: &GroupMenuRowContext<'_, 'a, M>,
    group: &WidgetGroupIdentity,
    item: GroupMenuItemRow<'a, M>,
    look: MenuRowLook,
) -> Element<'a, GroupEvent<M>> {
    let marked_target = item.inspected_target.clone();
    marked_by_hover_target(unmarked_item_row(layout, group, item, look), marked_target)
}

fn unmarked_item_row<'a, M: Clone + 'a>(
    layout: &GroupMenuRowContext<'_, 'a, M>,
    group: &WidgetGroupIdentity,
    item: GroupMenuItemRow<'a, M>,
    look: MenuRowLook,
) -> Element<'a, GroupEvent<M>> {
    let item_pin = match item.pin {
        GroupItemPin::Unpinnable => None,
        GroupItemPin::Pinnable(item_key) => Some(glyph_button(
            Phosphor::PushPin(Weight::Regular),
            layout.config.theme.font.icon_size_px,
            secondary_text_color,
            layout.menu.toggle_pin(GroupMenuPin::Item {
                group: group.clone(),
                item_key,
            }),
        )),
    };
    let inline_cards =
        layout.presentation.hover_card_placement == HoverCardPlacement::InlineWithRow;
    let card_disclosure = (inline_cards
        && matches!(item.gestures.left, GroupItemGesture::Performs { .. }))
    .then(|| {
        glyph_button(
            Phosphor::Info(Weight::Regular),
            layout.config.theme.font.icon_size_px,
            secondary_text_color,
            layout.menu.toggle_row_card(item.inspected_target.clone()),
        )
    });
    let content = MenuRowContent {
        leading: Vec::new(),
        parts: item.parts,
        name: item.name,
        trailing: item_pin.into_iter().chain(card_disclosure).collect(),
    };
    let row = match (layout.presentation.hover_card_placement, item.gestures.left) {
        (HoverCardPlacement::BesideRow, GroupItemGesture::Performs { message, .. }) => {
            hovered_menu_row(
                layout.config,
                content,
                look,
                GroupEvent::Group(message),
                layout.menu.inspect_row(item.inspected_target),
            )
        }
        (HoverCardPlacement::BesideRow, GroupItemGesture::Inert) => {
            let inspect = layout.menu.inspect_row(item.inspected_target);
            hovered_menu_row(layout.config, content, look, inspect.clone(), inspect)
        }
        (HoverCardPlacement::InlineWithRow, GroupItemGesture::Performs { message, .. }) => {
            pressable_menu_row(layout.config, content, look, GroupEvent::Group(message))
        }
        (HoverCardPlacement::InlineWithRow, GroupItemGesture::Inert) => pressable_menu_row(
            layout.config,
            content,
            look,
            layout.menu.toggle_row_card(item.inspected_target),
        ),
    };
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
