use std::borrow::Cow;

use iced::{
    Alignment, Color, Element, Length, Padding, Theme,
    border::Radius,
    widget::{Row, button, container, mouse_area, text::Wrapping},
};
use sottomano_toml_config::Config;

use crate::{
    group_items::group_item::MenuParts,
    group_menus::menu_rows::pointer_tracked_row::PointerTrackedRow,
    widget_groups::widget_group_content::GroupEvent,
    widgets::{
        configured_theme::{background_strong_color, primary_weak_color, text_color},
        flat_button,
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
}

#[derive(Clone, Copy)]
pub(crate) struct MenuRowLook {
    pub emphasis: MenuRowEmphasis,
    pub corners: Radius,
}

pub(crate) fn menu_row_height(config: &Config) -> f32 {
    config.desktop_panels.height + config.theme.spacing.item_gap
}

pub(crate) enum MenuRowPress<Message> {
    Inert,
    Performs(GroupEvent<Message>),
}

pub(crate) fn menu_row<'a, Message: Clone + 'a>(
    config: &'a Config,
    content: MenuRowContent<'a, Message>,
    look: MenuRowLook,
    press: MenuRowPress<Message>,
) -> Element<'a, GroupEvent<Message>> {
    let line = menu_line(config, content);
    let body: Element<'a, GroupEvent<Message>> = match press {
        MenuRowPress::Inert => container(line)
            .width(Length::Fill)
            .height(menu_row_height(config))
            .padding(menu_row_padding(config))
            .into(),
        MenuRowPress::Performs(press_event) => button(line)
            .width(Length::Fill)
            .height(menu_row_height(config))
            .padding(menu_row_padding(config))
            .style(flat_button(text_color))
            .on_press(press_event)
            .into(),
    };
    PointerTrackedRow::new(body, look).into()
}

pub(crate) fn hovered_menu_row<'a, Message: Clone + 'a>(
    config: &'a Config,
    content: MenuRowContent<'a, Message>,
    look: MenuRowLook,
    press_event: GroupEvent<Message>,
    pointer_entered: GroupEvent<Message>,
    pointer_left: GroupEvent<Message>,
) -> Element<'a, GroupEvent<Message>> {
    mouse_area(menu_row(
        config,
        content,
        look,
        MenuRowPress::Performs(press_event),
    ))
    .on_enter(pointer_entered)
    .on_exit(pointer_left)
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
    container(icon)
        .center_x(config.desktop_panels.height)
        .into()
}

fn menu_row_padding(config: &Config) -> Padding {
    Padding::ZERO.horizontal(config.theme.spacing.surface_inset)
}

pub(crate) fn menu_row_background(
    theme: &Theme,
    emphasis: MenuRowEmphasis,
    pointer_over: bool,
) -> Option<Color> {
    match (emphasis, pointer_over) {
        (MenuRowEmphasis::Selected, _) => Some(primary_weak_color(theme)),
        (MenuRowEmphasis::Plain, true) => Some(background_strong_color(theme)),
        (MenuRowEmphasis::Plain, false) => None,
    }
}

#[cfg(test)]
mod tests {
    use sottomano_toml_config::theme::palette::Palette;

    use super::{MenuRowEmphasis, menu_row_background};
    use crate::widgets::configured_theme::{
        background_strong_color, configured_theme, primary_weak_color,
    };

    #[test]
    fn a_selected_row_keeps_its_background_whether_or_not_the_pointer_is_over_it() {
        let theme = configured_theme(&Palette::default());
        let selected_color = Some(primary_weak_color(&theme));
        assert_eq!(
            menu_row_background(&theme, MenuRowEmphasis::Selected, false),
            selected_color
        );
        assert_eq!(
            menu_row_background(&theme, MenuRowEmphasis::Selected, true),
            selected_color
        );
    }

    #[test]
    fn a_plain_row_lights_up_only_under_the_pointer() {
        let theme = configured_theme(&Palette::default());
        assert_eq!(
            menu_row_background(&theme, MenuRowEmphasis::Plain, true),
            Some(background_strong_color(&theme))
        );
        assert_eq!(
            menu_row_background(&theme, MenuRowEmphasis::Plain, false),
            None
        );
    }
}
