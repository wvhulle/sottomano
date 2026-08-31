use iced::{Background, Border, Element, Length, widget::container};
use iced_layershell::actions::ActionCallback;

use crate::{
    app::Message,
    config::Config,
    wayland_layers::tray_menu::{menu_placement::anchored_menu, menu_surfaces::OpenMenu},
    widget_groups::tray_group::TrayMessage,
    widgets::{context_menu::context_menu, overlay_shadow::overlay_shadow},
};

const MENU_WIDTH: f32 = 250.0;

pub fn menu_content<'a>(open_menu: &OpenMenu, config: &'a Config) -> Element<'a, Message> {
    let entries = context_menu(
        config,
        &open_menu.nodes,
        &open_menu.submenu_open,
        &|id| Message::Tray(TrayMessage::MenuItemClicked(id)),
        &|id| Message::Tray(TrayMessage::ToggleSubmenu(id)),
    );
    let background = config.palette.surface;
    let radius = f32::from(config.chrome.popup_radius_px);
    let menu = container(entries)
        .width(Length::Fixed(MENU_WIDTH))
        .style(move |_| container::Style {
            background: Some(Background::Color(background)),
            border: Border {
                radius: radius.into(),
                ..Default::default()
            },
            shadow: overlay_shadow(1.0),
            ..Default::default()
        });
    let surface = open_menu.surface;
    anchored_menu(
        menu,
        open_menu.anchor_x,
        open_menu.position,
        config.chrome.outer_height_px + config.chrome.popup_gap_px,
        move |bounds| Message::SetInputRegion {
            id: surface,
            callback: ActionCallback::new(move |region| {
                region.add(
                    bounds.x as i32,
                    bounds.y as i32,
                    bounds.width as i32,
                    bounds.height as i32,
                );
            }),
        },
    )
}
