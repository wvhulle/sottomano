use iced::{
    Alignment, Background, Border, Element, Length, Padding, Theme,
    border::Radius,
    widget::{Row, button, container, mouse_area, space},
};
use sottomano_config::Config;

use crate::{
    hover_tooltips::{InspectedRow, TooltipMessage},
    widgets::{
        configured_theme::{primary_weak_color, text_color},
        group_item::MenuParts,
        text::bar_text,
        widget_group::GroupEvent,
    },
};

pub(super) struct MenuRowContent<'a, M> {
    pub parts: MenuParts<'a, M>,
    pub name: &'static str,
    pub trailing: Vec<Element<'a, GroupEvent<M>>>,
}

pub(super) struct MenuRowAction<M> {
    pub message: GroupEvent<M>,
    pub on_enter: Option<GroupEvent<M>>,
    pub highlighted: bool,
    pub corners: Radius,
}

pub(super) fn inspect_row_message<M>(inspected_row: InspectedRow) -> GroupEvent<M> {
    GroupEvent::Tooltip(TooltipMessage::Inspect(inspected_row))
}

pub(super) fn menu_line<'a, M: 'a>(
    config: &'a Config,
    content: MenuRowContent<'a, M>,
) -> Row<'a, GroupEvent<M>> {
    let MenuRowContent {
        parts,
        name,
        trailing,
    } = content;
    Row::new()
        .push(leading_icon_slot(config, parts.icon))
        .push(bar_text(&config.theme.font, name))
        .push(space::horizontal())
        .push(parts.value)
        .extend(trailing)
        .spacing(config.layout.overlay.column_spacing)
        .height(Length::Fill)
        .align_y(Alignment::Center)
}

fn leading_icon_slot<'a, M: 'a>(
    config: &Config,
    icon: Element<'a, GroupEvent<M>>,
) -> Element<'a, GroupEvent<M>> {
    let slot_width = config.layout.panel.height;
    let glyph_inset = (slot_width - config.theme.font.icon_size_px) / 2.0;
    container(icon)
        .width(slot_width)
        .padding(Padding::ZERO.left(glyph_inset))
        .into()
}

pub(super) fn menu_row_height(config: &Config) -> f32 {
    config.layout.panel.height + config.layout.overlay.row_spacing
}

pub(super) fn menu_row_padding(config: &Config) -> Padding {
    Padding::ZERO.horizontal(config.layout.overlay.content_inset)
}

pub(super) fn inert_menu_row<'a, M: 'a>(
    config: &'a Config,
    content: MenuRowContent<'a, M>,
) -> Element<'a, GroupEvent<M>> {
    container(menu_line(config, content))
        .width(Length::Fill)
        .height(menu_row_height(config))
        .padding(menu_row_padding(config))
        .into()
}

pub(super) fn menu_row<'a, M: Clone + 'a>(
    config: &'a Config,
    content: MenuRowContent<'a, M>,
    action: MenuRowAction<M>,
) -> Element<'a, GroupEvent<M>> {
    let MenuRowAction {
        message,
        on_enter,
        highlighted,
        corners,
    } = action;
    let pressable = button(menu_line(config, content))
        .width(Length::Fill)
        .height(menu_row_height(config))
        .padding(menu_row_padding(config))
        .style(move |theme, status| highlighted_row_style(theme, status, highlighted, corners))
        .on_press(message);
    match on_enter {
        Some(on_enter) => mouse_area(pressable).on_enter(on_enter).into(),
        None => pressable.into(),
    }
}

pub(super) fn highlighted_row_style(
    theme: &Theme,
    status: button::Status,
    open: bool,
    corners: Radius,
) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: (open || hovered).then(|| Background::Color(primary_weak_color(theme))),
        text_color: text_color(theme),
        border: Border {
            radius: corners,
            ..Border::default()
        },
        ..button::Style::default()
    }
}
