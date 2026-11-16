use corredo::{
    popovers::{
        BesideRow, CascadeDirection, NestedMenuItem, NestedMenuLayout, NestedMenuPresentation,
        NestedMenuRow, NestedMenuRowEmphasis, RowGesture, RowHoverReaction, SatelliteReach,
    },
    theming::{primary_weak_text_color, secondary_text_color, text_color},
};
use iced::mouse::ScrollDelta;
use sottomano_toml_config::Config;

use crate::{
    clock::Clock,
    group_indicators::{
        group_indicator::{GroupIndicator, RowScroll},
        indicator_gestures::IndicatorGesture,
    },
    group_menus::{
        group_menu_state::{
            GroupMenuMessage, GroupMenuPin, GroupMenuState, IndicatorPin, PanelPin,
            PanelPinDuration,
        },
        group_menu_view::{GroupMenuMessenger, menu_event},
        menu_rows::menu_row::menu_row_content,
    },
    hover_cards::{IndicatorKey, marked_by_menu_pin},
    widget_groups::{
        group_summary::GroupSummary,
        widget_group_content::GroupEvent,
        widget_group_identity::{WidgetGroupIdentity, WidgetGroupTitle},
    },
    widgets::{
        glyph_button,
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
};

pub(crate) struct GroupMenuGroupRow<'a> {
    pub group_identity: WidgetGroupIdentity,
    pub group_title: WidgetGroupTitle,
    pub summary: GroupSummary<'a>,
    pub items: Vec<GroupIndicator<'a>>,
}

pub(crate) fn one_row_per_identity<'a>(
    rows: impl IntoIterator<Item = GroupMenuGroupRow<'a>>,
) -> Vec<GroupMenuGroupRow<'a>> {
    rows.into_iter().fold(Vec::new(), |mut merged, row| {
        match merged
            .iter_mut()
            .find(|existing: &&mut GroupMenuGroupRow<'a>| {
                existing.group_identity == row.group_identity
            }) {
            Some(existing) => existing.items.extend(row.items),
            None => merged.push(row),
        }
        merged
    })
}

pub(crate) struct GroupMenuRowContext<'p> {
    pub config: &'p Config,
    pub direction: CascadeDirection,
    pub presentation: NestedMenuPresentation,
    pub menu: &'p GroupMenuMessenger,
    pub group_menus: &'p GroupMenuState,
    pub clock: Clock,
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

pub(crate) fn group_header<'a>(
    layout: &GroupMenuRowContext<'_>,
    entry_identity: &WidgetGroupIdentity,
    group_title: &WidgetGroupTitle,
    summary: &GroupSummary<'a>,
    emphasis: NestedMenuRowEmphasis,
) -> NestedMenuRow<'a, GroupEvent> {
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
        leading,
        summary.menu_icon(layout.config),
        group_title.to_string().into(),
        summary.menu_value(layout.config, layout.clock),
        trailing,
    );
    let group_identity = entry_identity.clone();
    let (press, hover_reaction) = match layout.presentation.layout {
        NestedMenuLayout::Cascading => (
            layout.menu.reveal_group(group_identity.clone()),
            RowHoverReaction::Reveals {
                reveal: layout.menu.reveal_group(group_identity.clone()),
                defer_reveal: layout.menu.intend_group_reveal(group_identity.clone()),
                cancel_reveal: menu_event(GroupMenuMessage::CancelRevealIntent { group_identity }),
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
        press: RowGesture::Publishes(press),
        hover_reaction,
        entered: RowGesture::Inert,
        right_pressed: RowGesture::Inert,
        scrolled: RowGesture::Inert,
    }
}

pub(crate) fn item_entry<'a>(
    layout: &GroupMenuRowContext<'_>,
    group_identity: &WidgetGroupIdentity,
    item: &GroupIndicator<'a>,
    beside: BesideRow<'a, GroupEvent>,
) -> NestedMenuItem<'a, IndicatorKey, GroupEvent> {
    let emphasis = item.row_emphasis(
        group_identity,
        layout.group_menus.inspected_indicator(layout.menu.topic),
    );
    let indicator = item.indicator_key(group_identity);
    let item_pin = layout.group_menus.item_pin(
        layout.menu.topic,
        group_identity,
        item,
        layout.config.group_menus.alert_promotion_severity,
    );
    let pin_button = |entry, pin_weight, pin_color: fn(&iced::Theme) -> iced::Color| {
        glyph_button(
            Phosphor::PushPin(pin_weight),
            layout.config.theme.font.icon_size_px,
            pin_color,
            layout.menu.toggle_pin(entry),
        )
    };
    let item_pin = match item_pin {
        IndicatorPin::Unpinnable => None,
        IndicatorPin::Pinned(PanelPin {
            entry,
            duration: PanelPinDuration::Permanent,
            ..
        }) => Some(pin_button(
            entry,
            Weight::Fill,
            pinned_glyph_color(emphasis),
        )),
        IndicatorPin::Unpinned(entry)
        | IndicatorPin::Pinned(PanelPin {
            entry,
            duration: PanelPinDuration::UntilAlertEnds,
            ..
        }) => Some(pin_button(
            entry,
            Weight::Regular,
            trailing_glyph_color(emphasis),
        )),
    };
    let content = menu_row_content(
        layout.config,
        Vec::new(),
        item.menu_icon(layout.config),
        item.indicator_name(),
        item.menu_row_value(layout.config, layout.clock),
        item_pin.into_iter().collect(),
    );
    let gestures = item.gestures();
    let press = match gestures.left {
        IndicatorGesture::Performs { message, .. } => {
            RowGesture::Publishes(GroupEvent::Interaction(message))
        }
        IndicatorGesture::Inert => RowGesture::Inert,
    };
    let right_pressed = match gestures.right {
        IndicatorGesture::Performs { message, .. } => {
            RowGesture::Publishes(GroupEvent::Interaction(message))
        }
        IndicatorGesture::Inert => RowGesture::Inert,
    };
    let scrolled = match item.row_scroll() {
        RowScroll::AdjustsValue(adjust_by_scroll) => RowGesture::Publishes(Box::new(move |delta| {
            GroupEvent::Interaction(adjust_by_scroll(delta))
        })
            as Box<dyn Fn(ScrollDelta) -> GroupEvent + 'a>),
        RowScroll::Inert => RowGesture::Inert,
    };
    NestedMenuItem {
        key: indicator.clone(),
        row: NestedMenuRow {
            content,
            emphasis,
            press,
            hover_reaction: RowHoverReaction::Highlights,
            entered: RowGesture::Publishes(layout.menu.inspect_row(indicator)),
            right_pressed,
            scrolled,
        },
        beside,
    }
}

#[cfg(test)]
mod tests {
    use super::{GroupMenuGroupRow, WidgetGroupIdentity, one_row_per_identity};
    use crate::{
        group_indicators::indicator_state::IndicatorState,
        widget_groups::{
            group_summary::GroupSummary,
            widget_group_identity::{WidgetGroupFamily, WidgetGroupTitle},
        },
        widgets::icon_glyph::IconGlyph,
    };

    fn row_of(group_identity: WidgetGroupIdentity, title: &str) -> GroupMenuGroupRow<'static> {
        GroupMenuGroupRow {
            group_identity,
            group_title: WidgetGroupTitle::Named(title.to_owned().into()),
            summary: GroupSummary::glyph(IconGlyph::default(), IndicatorState::Idle),
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
