use iced::{
    Alignment, Background, Border, Element, Length, Padding, Theme,
    border::Radius,
    widget::{Row, button, container, mouse_area, space},
};
use sottomano_config::Config;

use crate::widgets::{
    configured_theme::{primary_weak_color, text_color},
    group_item::MenuParts,
    text::bar_text,
    widget_group::GroupEvent,
};

pub(super) struct MenuRowContent<'a, M> {
    pub parts: MenuParts<'a, M>,
    pub name: &'static str,
    pub trailing: Vec<Element<'a, GroupEvent<M>>>,
}

#[derive(Clone, Copy)]
pub(super) struct MenuRowLook {
    pub highlighted: bool,
    pub corners: Radius,
}

pub(super) fn menu_row_height(config: &Config) -> f32 {
    config.layout.panel.height + config.layout.overlay.row_spacing
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

pub(super) fn pressable_menu_row<'a, M: Clone + 'a>(
    config: &'a Config,
    content: MenuRowContent<'a, M>,
    look: MenuRowLook,
    press: GroupEvent<M>,
) -> Element<'a, GroupEvent<M>> {
    button(menu_line(config, content))
        .width(Length::Fill)
        .height(menu_row_height(config))
        .padding(menu_row_padding(config))
        .style(move |theme, status| menu_row_style(theme, status, look))
        .on_press(press)
        .into()
}

pub(super) fn hovered_menu_row<'a, M: Clone + 'a>(
    config: &'a Config,
    content: MenuRowContent<'a, M>,
    look: MenuRowLook,
    press: GroupEvent<M>,
    hover: GroupEvent<M>,
) -> Element<'a, GroupEvent<M>> {
    mouse_area(pressable_menu_row(config, content, look, press))
        .on_enter(hover)
        .into()
}

fn menu_line<'a, M: 'a>(
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

fn menu_row_padding(config: &Config) -> Padding {
    Padding::ZERO.horizontal(config.layout.overlay.content_inset)
}

fn menu_row_style(theme: &Theme, status: button::Status, look: MenuRowLook) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: (look.highlighted || hovered)
            .then(|| Background::Color(primary_weak_color(theme))),
        text_color: text_color(theme),
        border: Border {
            radius: look.corners,
            ..Border::default()
        },
        ..button::Style::default()
    }
}
