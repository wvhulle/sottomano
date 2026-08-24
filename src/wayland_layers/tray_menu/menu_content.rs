use iced::{
    Background, Border, Element, Length,
    widget::{Space, container, mouse_area, stack},
};

use crate::{
    app::Message,
    config::Config,
    wayland_layers::tray_menu::{menu_placement::anchored_menu, menu_surfaces::OpenMenu},
    widget_groups::tray_group::TrayMessage,
    widgets::context_menu::context_menu,
};

const MENU_WIDTH: f32 = 250.0;
const MENU_PADDING: f32 = 6.0;
const BORDER_ALPHA: f32 = 0.25;

pub fn menu_content<'a>(open_menu: &OpenMenu, config: &'a Config) -> Element<'a, Message> {
    let entries = context_menu(
        config,
        &open_menu.nodes,
        &open_menu.submenu_open,
        &|id| Message::Tray(TrayMessage::MenuItemClicked(id)),
        &|id| Message::Tray(TrayMessage::ToggleSubmenu(id)),
    );
    let background = config.palette.popup_background();
    let menu = container(entries)
        .width(Length::Fixed(MENU_WIDTH))
        .padding([MENU_PADDING, 0.0])
        .style(move |_| container::Style {
            background: Some(Background::Color(background)),
            border: Border {
                color: config.palette.bar_foreground.scale_alpha(BORDER_ALPHA),
                width: 1.0,
                radius: f32::from(config.chrome.popup_radius_px).into(),
            },
            ..Default::default()
        });
    let dismiss = mouse_area(
        container(Space::new())
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(Message::DismissPressed);
    stack![
        dismiss,
        anchored_menu(
            menu,
            open_menu.anchor_x,
            open_menu.position,
            config.chrome.popup_gap_px
        ),
    ]
    .into()
}
