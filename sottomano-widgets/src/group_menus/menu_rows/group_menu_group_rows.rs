use corredo::{
    popovers::{
        BesideRow, CascadeDirection, NestedMenuItem, NestedMenuLayout, NestedMenuPresentation,
        NestedMenuRow, NestedMenuRowEmphasis, NestedMenuRowPress, RowGesture, RowHoverReaction,
        SatelliteReach,
    },
    theming::{primary_weak_text_color, secondary_text_color, text_color},
};
use iced::mouse::ScrollDelta;
use sottomano_toml_config::Config;

use crate::{
    group_items::{
        group_item::{MenuParts, RowScroll},
        group_item_gestures::GroupItemGesture,
    },
    group_menus::{
        group_menu_state::{GroupMenuPin, GroupMenuState},
        group_menu_view::GroupMenuMessenger,
        menu_rows::menu_row::{MenuRowContent, menu_row_content},
    },
    hover_cards::{HoverTarget, marked_by_menu_pin},
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

pub(crate) struct GroupMenuRowContext<'p, M> {
    pub config: &'p Config,
    pub direction: CascadeDirection,
    pub presentation: NestedMenuPresentation,
    pub menu: &'p GroupMenuMessenger<M>,
    pub group_menus: &'p GroupMenuState,
}

const fn accordion_caret(emphasis: NestedMenuRowEmphasis) -> Phosphor {
    match emphasis {
        NestedMenuRowEmphasis::Plain => Phosphor::CaretRight(Weight::Regular),
        NestedMenuRowEmphasis::Opened | NestedMenuRowEmphasis::Selected => {
            Phosphor::CaretDown(Weight::Regular)
        }
    }
}

fn trailing_glyph_color(emphasis: NestedMenuRowEmphasis) -> fn(&iced::Theme) -> iced::Color {
    match emphasis {
        NestedMenuRowEmphasis::Plain | NestedMenuRowEmphasis::Opened => secondary_text_color,
        NestedMenuRowEmphasis::Selected => primary_weak_text_color,
    }
}

fn pinned_glyph_color(emphasis: NestedMenuRowEmphasis) -> fn(&iced::Theme) -> iced::Color {
    match emphasis {
        NestedMenuRowEmphasis::Plain | NestedMenuRowEmphasis::Opened => text_color,
        NestedMenuRowEmphasis::Selected => primary_weak_text_color,
    }
}

pub(crate) fn group_header<'a, M: Clone + From<GroupEvent<M>> + 'a>(
    layout: &GroupMenuRowContext<'_, M>,
    entry_identity: &WidgetGroupIdentity,
    group_title: &WidgetGroupTitle,
    face: MenuParts<'a, M>,
    emphasis: NestedMenuRowEmphasis,
) -> NestedMenuRow<'a, M> {
    let icon_size = layout.config.theme.font.icon_size_px;
    let pinned_entry = GroupMenuPin::Group(entry_identity.clone());
    let pin = marked_by_menu_pin(
        glyph_button(
            Phosphor::PushPin(Weight::Regular),
            icon_size,
            trailing_glyph_color(emphasis),
            layout.menu.toggle_pin(pinned_entry.clone()),
        ),
        pinned_entry,
    );
    let caret_color = trailing_glyph_color(emphasis);
    let (leading, trailing) = match (layout.presentation.layout, layout.direction) {
        (NestedMenuLayout::Accordion, _) => (
            Vec::new(),
            vec![
                pin,
                IconGlyph::from(accordion_caret(emphasis)).view(icon_size, caret_color),
            ],
        ),
        (NestedMenuLayout::Cascading, CascadeDirection::Rightward) => (
            Vec::new(),
            vec![
                pin,
                IconGlyph::from(Phosphor::CaretRight(Weight::Regular)).view(icon_size, caret_color),
            ],
        ),
        (NestedMenuLayout::Cascading, CascadeDirection::Leftward) => (
            vec![
                IconGlyph::from(Phosphor::CaretLeft(Weight::Regular)).view(icon_size, caret_color),
            ],
            vec![pin],
        ),
    };
    let content = menu_row_content(
        layout.config,
        MenuRowContent {
            leading,
            parts: face,
            name: group_title.to_string().into(),
            trailing,
        },
    );
    let group_identity = entry_identity.clone();
    let (press, hover_reaction) = match layout.presentation.layout {
        NestedMenuLayout::Cascading => (
            layout.menu.reveal_group(group_identity.clone()),
            RowHoverReaction::Reveals {
                reveal: M::from(layout.menu.reveal_group(group_identity.clone())),
                defer_reveal: M::from(layout.menu.intend_group_reveal(group_identity.clone())),
                cancel_reveal: M::from(layout.menu.cancel_group_reveal(group_identity)),
                satellite_reach: if layout.group_menus.open_groups(layout.menu.topic).is_empty() {
                    SatelliteReach::Closed
                } else {
                    SatelliteReach::Open(layout.direction)
                },
            },
        ),
        NestedMenuLayout::Accordion => (
            layout.menu.toggle_group(group_identity),
            RowHoverReaction::Highlights,
        ),
    };
    NestedMenuRow {
        content,
        emphasis,
        press: NestedMenuRowPress::Performs(M::from(press)),
        hover_reaction,
        entered: RowGesture::Inert,
        right_pressed: RowGesture::Inert,
        scrolled: RowGesture::Inert,
    }
}

pub(crate) fn item_entry<'a, M: Clone + From<GroupEvent<M>> + 'a>(
    layout: &GroupMenuRowContext<'_, M>,
    group: &WidgetGroupIdentity,
    item: GroupMenuItemRow<'a, M>,
    beside: BesideRow<'a, M>,
) -> NestedMenuItem<'a, HoverTarget, M> {
    let emphasis = NestedMenuRowEmphasis::from(
        item.inspection(layout.group_menus.inspected_target(layout.menu.topic)),
    );
    let pin_button = |pin_weight, pin_color: fn(&iced::Theme) -> iced::Color| {
        glyph_button(
            Phosphor::PushPin(pin_weight),
            layout.config.theme.font.icon_size_px,
            pin_color,
            layout.menu.toggle_pin(GroupMenuPin::Item {
                group: group.clone(),
                item_in_group_name: item.name.to_string(),
            }),
        )
    };
    let item_pin = match item.pin {
        GroupItemPin::Unpinnable => None,
        GroupItemPin::Unpinned => Some(pin_button(Weight::Regular, trailing_glyph_color(emphasis))),
        GroupItemPin::Pinned => Some(pin_button(Weight::Fill, pinned_glyph_color(emphasis))),
    };
    let content = menu_row_content(
        layout.config,
        MenuRowContent {
            leading: Vec::new(),
            parts: item.parts,
            name: item.name,
            trailing: item_pin.into_iter().collect(),
        },
    );
    let press = match item.gestures.left {
        GroupItemGesture::Performs { message, .. } => {
            NestedMenuRowPress::Performs(M::from(GroupEvent::Group(message)))
        }
        GroupItemGesture::Inert => NestedMenuRowPress::Inert,
    };
    let right_pressed = match item.gestures.right {
        GroupItemGesture::Performs { message, .. } => {
            RowGesture::Publishes(M::from(GroupEvent::Group(message)))
        }
        GroupItemGesture::Inert => RowGesture::Inert,
    };
    let scrolled = match item.row_scroll {
        RowScroll::AdjustsValue(adjust_by_scroll) => RowGesture::Publishes(Box::new(move |delta| {
            M::from(GroupEvent::Group(adjust_by_scroll(delta)))
        })
            as Box<dyn Fn(ScrollDelta) -> M + 'a>),
        RowScroll::Inert => RowGesture::Inert,
    };
    NestedMenuItem {
        key: item.inspected_target.clone(),
        row: NestedMenuRow {
            content,
            emphasis,
            press,
            hover_reaction: RowHoverReaction::Highlights,
            entered: RowGesture::Publishes(M::from(layout.menu.inspect_row(item.inspected_target))),
            right_pressed,
            scrolled,
        },
        beside,
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
