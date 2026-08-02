use iced::{
    Alignment, Element,
    widget::{Row, mouse_area},
};

use crate::{
    config::resolved::ResolvedConfig,
    data::source::font_awesome::{FontAwesome, Solid},
    wayland_layers::surface::Position,
    widgets::pager_slot::permanently_inactive_slot,
};

#[derive(Debug, Clone, Copy)]
pub enum NotificationGroupMessage {
    ToggleDoNotDisturb,
    ToggleSound,
}

pub fn view<'a>(
    config: &ResolvedConfig,
    position: Position,
) -> Element<'a, NotificationGroupMessage> {
    let do_not_disturb = toggle(
        config,
        if config.dnd {
            Solid::BellSlash.into()
        } else {
            Solid::Bell.into()
        },
        !config.dnd,
        NotificationGroupMessage::ToggleDoNotDisturb,
    );
    let sound = toggle(
        config,
        if config.sound {
            Solid::VolumeHigh.into()
        } else {
            Solid::VolumeXmark.into()
        },
        config.sound,
        NotificationGroupMessage::ToggleSound,
    );

    let toggles = Row::new()
        .push(do_not_disturb)
        .push(sound)
        .spacing(config.layout.horizontal_widget_spacing_px)
        .align_y(Alignment::Center);
    permanently_inactive_slot(toggles.into(), config, position)
}

fn toggle<'a>(
    config: &ResolvedConfig,
    icon: FontAwesome,
    active: bool,
    message: NotificationGroupMessage,
) -> Element<'a, NotificationGroupMessage> {
    let color = config
        .palette
        .bar_foreground
        .scale_alpha(if active { 1.0 } else { 0.4 });
    mouse_area(icon.widget().size(config.font.icon_size_px).color(color))
        .on_press(message)
        .into()
}
