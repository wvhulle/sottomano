use core::iter::repeat_n;
use std::{borrow::Cow, time::Duration};

use iced::{
    Alignment, Element, Length, Padding,
    border::Radius,
    mouse::ScrollDelta,
    widget::{Row, Space, button, container, mouse_area, text, text::Wrapping},
};

mod pointer_tracked_row;
mod row_emphasis;

use pointer_tracked_row::PointerTrackedRow;
pub use row_emphasis::MenuRowEmphasis;

use crate::{popovers::CascadeDirection, press_actions::flat_button, theming::TextFace};

pub(crate) enum RowGesture<Payload> {
    Inert,
    Publishes(Payload),
}

type ScrollAdjustment<'view, Message> = Box<dyn Fn(ScrollDelta) -> Message + 'view>;

pub struct MenuRow<'view, Message> {
    pub(crate) content: MenuRowContent<'view, Message>,
    pub(crate) emphasis: MenuRowEmphasis,
    pub(crate) press: RowGesture<Message>,
    pub(crate) hover_reaction: RowHoverReaction<Message>,
    entered: RowGesture<Message>,
    right_pressed: RowGesture<Message>,
    scrolled: RowGesture<ScrollAdjustment<'view, Message>>,
}

impl<'view, Message> MenuRow<'view, Message> {
    #[must_use]
    pub fn new(content: MenuRowContent<'view, Message>) -> Self {
        Self {
            content,
            emphasis: MenuRowEmphasis::Plain,
            press: RowGesture::Inert,
            hover_reaction: RowHoverReaction::Highlights,
            entered: RowGesture::Inert,
            right_pressed: RowGesture::Inert,
            scrolled: RowGesture::Inert,
        }
    }

    #[must_use]
    pub fn emphasis(self, emphasis: MenuRowEmphasis) -> Self {
        Self { emphasis, ..self }
    }

    #[must_use]
    pub fn on_press(self, message: Message) -> Self {
        Self {
            press: RowGesture::Publishes(message),
            ..self
        }
    }

    #[must_use]
    pub fn on_enter(self, message: Message) -> Self {
        Self {
            entered: RowGesture::Publishes(message),
            ..self
        }
    }

    #[must_use]
    pub fn on_right_press(self, message: Message) -> Self {
        Self {
            right_pressed: RowGesture::Publishes(message),
            ..self
        }
    }

    #[must_use]
    pub(crate) fn hover_reaction(self, hover_reaction: RowHoverReaction<Message>) -> Self {
        Self {
            hover_reaction,
            ..self
        }
    }

    #[must_use]
    pub fn on_scroll(self, adjust: impl Fn(ScrollDelta) -> Message + 'view) -> Self {
        Self {
            scrolled: RowGesture::Publishes(Box::new(adjust)),
            ..self
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct MenuRowLook {
    pub emphasis: MenuRowEmphasis,
    pub frame_corners: Radius,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MenuRowMetrics {
    pub face: TextFace,
    pub row_height_px: f32,
    pub icon_slot_px: f32,
    pub item_spacing_px: f32,
    pub side_inset_px: f32,
    pub indent_step_px: f32,
}

impl Default for MenuRowMetrics {
    fn default() -> Self {
        Self {
            face: TextFace::default(),
            row_height_px: 28.0,
            icon_slot_px: 24.0,
            item_spacing_px: 8.0,
            side_inset_px: 8.0,
            indent_step_px: 14.0,
        }
    }
}

pub(crate) enum RowIcon<'view, Message> {
    Absent,
    Blank,
    Shown(Element<'view, Message>),
}

impl<'view, Message> RowIcon<'view, Message> {
    fn within(self, column: IconColumn) -> Self {
        match (column, self) {
            (IconColumn::Reserved, RowIcon::Absent) => RowIcon::Blank,
            (IconColumn::Reserved | IconColumn::Omitted, slot) => slot,
        }
    }

    fn slot_view(self, slot_width_px: f32) -> Option<Element<'view, Message>>
    where
        Message: 'view,
    {
        match self {
            RowIcon::Absent => None,
            RowIcon::Blank => Some(Space::new().width(slot_width_px).into()),
            RowIcon::Shown(glyph) => Some(container(glyph).center_x(slot_width_px).into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IconColumn {
    Reserved,
    Omitted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IconSide {
    Leading,
    Trailing,
}

pub struct MenuRowContent<'view, Message> {
    pub(crate) indent_levels: usize,
    pub(crate) leading_caret: RowIcon<'view, Message>,
    pub(crate) icon: RowIcon<'view, Message>,
    icon_side: IconSide,
    label: Cow<'view, str>,
    value: Element<'view, Message>,
    pub(crate) trailing: Vec<Element<'view, Message>>,
}

impl<'view, Message: 'view> MenuRowContent<'view, Message> {
    #[must_use]
    pub fn new(label: impl Into<Cow<'view, str>>) -> Self {
        Self {
            indent_levels: 0,
            leading_caret: RowIcon::Absent,
            icon: RowIcon::Absent,
            icon_side: IconSide::Leading,
            label: label.into(),
            value: Space::new().into(),
            trailing: Vec::new(),
        }
    }

    #[must_use]
    pub fn icon(self, icon: impl Into<Element<'view, Message>>) -> Self {
        Self {
            icon: RowIcon::Shown(icon.into()),
            ..self
        }
    }

    pub(crate) fn within(
        self,
        icon_column: IconColumn,
        caret_column: IconColumn,
        icon_side: IconSide,
    ) -> Self {
        Self {
            icon: self.icon.within(icon_column),
            icon_side,
            leading_caret: self.leading_caret.within(caret_column),
            ..self
        }
    }

    #[must_use]
    pub fn value(self, value: impl Into<Element<'view, Message>>) -> Self {
        Self {
            value: value.into(),
            ..self
        }
    }

    #[must_use]
    pub fn trailing(self, trailing: impl IntoIterator<Item = Element<'view, Message>>) -> Self {
        Self {
            trailing: trailing.into_iter().collect(),
            ..self
        }
    }
}

fn drawn_menu_row<'view, Message: Clone + 'view>(
    metrics: MenuRowMetrics,
    content: MenuRowContent<'view, Message>,
    look: MenuRowLook,
    press: RowGesture<Message>,
    hover_reaction: RowHoverReaction<Message>,
) -> Element<'view, Message> {
    let MenuRowContent {
        indent_levels,
        leading_caret,
        icon,
        icon_side,
        label,
        value,
        trailing,
    } = content;
    let label_color = look.emphasis.text_color();
    let label_text = metrics
        .face
        .text(label)
        .wrapping(Wrapping::None)
        .style(move |theme| text::Style {
            color: Some(label_color(theme)),
        });
    let icon_slot = icon.slot_view(metrics.icon_slot_px);
    let (leading_icon, trailing_icon) = match icon_side {
        IconSide::Leading => (icon_slot, None),
        IconSide::Trailing => (None, icon_slot),
    };
    let line = Row::new()
        .extend(
            repeat_n((), indent_levels).map(|()| Space::new().width(metrics.indent_step_px).into()),
        )
        .extend(leading_caret.slot_view(metrics.icon_slot_px))
        .extend(leading_icon)
        .push(container(label_text).width(Length::Fill).clip(true))
        .push(value)
        .extend(trailing)
        .extend(trailing_icon)
        .spacing(metrics.item_spacing_px)
        .height(Length::Fill)
        .align_y(Alignment::Center);
    let padding = Padding::ZERO.horizontal(metrics.side_inset_px);
    let body: Element<'view, Message> = match press {
        RowGesture::Inert => container(line)
            .width(Length::Fill)
            .height(metrics.row_height_px)
            .padding(padding)
            .into(),
        RowGesture::Publishes(press_message) => button(line)
            .width(Length::Fill)
            .height(metrics.row_height_px)
            .padding(padding)
            .style(flat_button(label_color))
            .on_press(press_message)
            .into(),
    };
    Element::new(PointerTrackedRow {
        body,
        look,
        hover_reaction,
    })
}

pub(crate) fn gestured_menu_row<'view, Message: Clone + 'view>(
    row: MenuRow<'view, Message>,
    metrics: MenuRowMetrics,
    frame_corners: Radius,
) -> Element<'view, Message> {
    let MenuRow {
        content,
        emphasis,
        press,
        hover_reaction,
        entered,
        right_pressed,
        scrolled,
    } = row;
    let drawn = drawn_menu_row(
        metrics,
        content,
        MenuRowLook {
            emphasis,
            frame_corners,
        },
        press,
        hover_reaction,
    );
    let gestured = mouse_area(drawn);
    let gestured = match entered {
        RowGesture::Publishes(message) => gestured.on_enter(message),
        RowGesture::Inert => gestured,
    };
    let gestured = match right_pressed {
        RowGesture::Publishes(message) => gestured.on_right_press(message),
        RowGesture::Inert => gestured,
    };
    match scrolled {
        RowGesture::Publishes(adjust) => gestured.on_scroll(adjust).into(),
        RowGesture::Inert => gestured.into(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SatelliteReach {
    Open(CascadeDirection),
    Closed,
}

pub(crate) enum RowHoverReaction<Message> {
    Highlights,
    Reveals {
        reveal: Message,
        satellite_reach: SatelliteReach,
        reveal_delay: Duration,
    },
}
