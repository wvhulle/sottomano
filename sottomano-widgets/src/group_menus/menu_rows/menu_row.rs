use std::borrow::Cow;

use iced::{
    Alignment, Background, Border, Element, Length, Padding, Theme,
    border::Radius,
    widget::{Row, button, container, mouse_area, text::Wrapping},
};
use sottomano_config::Config;

use crate::{
    group_items::group_item::MenuParts,
    widget_groups::widget_group_content::GroupEvent,
    widgets::{
        configured_theme::{background_weak_color, primary_weak_color, text_color},
        text::panel_text,
    },
};

pub(crate) struct MenuRowContent<'a, M> {
    pub leading: Vec<Element<'a, GroupEvent<M>>>,
    pub parts: MenuParts<'a, M>,
    pub name: Cow<'static, str>,
    pub trailing: Vec<Element<'a, GroupEvent<M>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuRowEmphasis {
    Plain,
    OpenGroup,
    Selected,
}

impl MenuRowEmphasis {
    pub(crate) const fn selected_when(selected: bool) -> Self {
        if selected {
            Self::Selected
        } else {
            Self::Plain
        }
    }

    pub(crate) const fn open_group_when(open: bool) -> Self {
        if open { Self::OpenGroup } else { Self::Plain }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct MenuRowLook {
    pub emphasis: MenuRowEmphasis,
    pub corners: Radius,
}

pub(crate) fn menu_row_height(config: &Config) -> f32 {
    config.desktop_panels.height + config.theme.spacing.item_gap
}

pub(crate) fn pressable_menu_row<'a, M: Clone + 'a>(
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

pub(crate) fn hovered_menu_row<'a, M: Clone + 'a>(
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
        leading,
        parts,
        name,
        trailing,
    } = content;
    Row::new()
        .extend(leading)
        .push(leading_icon_slot(config, parts.icon))
        .push(
            container(panel_text(&config.theme.font, name).wrapping(Wrapping::None))
                .width(Length::Fill)
                .clip(true),
        )
        .push(parts.value)
        .extend(trailing)
        .spacing(config.theme.spacing.group_gap)
        .height(Length::Fill)
        .align_y(Alignment::Center)
}

fn leading_icon_slot<'a, M: 'a>(
    config: &Config,
    icon: Element<'a, GroupEvent<M>>,
) -> Element<'a, GroupEvent<M>> {
    container(icon).width(config.desktop_panels.height).into()
}

fn menu_row_padding(config: &Config) -> Padding {
    Padding::ZERO.horizontal(config.theme.spacing.surface_inset)
}

fn menu_row_style(theme: &Theme, status: button::Status, look: MenuRowLook) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: match (look.emphasis, hovered) {
            (MenuRowEmphasis::Selected, _) | (_, true) => {
                Some(Background::Color(primary_weak_color(theme)))
            }
            (MenuRowEmphasis::OpenGroup, false) => {
                Some(Background::Color(background_weak_color(theme)))
            }
            (MenuRowEmphasis::Plain, false) => None,
        },
        text_color: text_color(theme),
        border: Border {
            radius: look.corners,
            ..Border::default()
        },
        ..button::Style::default()
    }
}
