use iced::{
    Alignment, Element,
    widget::{Row, button},
};

use crate::{
    config::Config,
    data::source::font_awesome::{FontAwesome, Regular, Solid},
    state::NotificationState,
    wayland_layers::surface::Position,
    widgets::{DISABLED_ALPHA, flat_button, pager_slot::permanently_inactive_slot},
};

#[derive(Debug, Clone, Copy)]
pub enum NotificationGroupMessage {
    DoNotDisturb,
    Sound,
    SpatialLayout,
}

pub fn view<'a>(
    config: &Config,
    notif_state: &NotificationState,
    position: Position,
    any_pending: bool,
) -> Element<'a, NotificationGroupMessage> {
    let do_not_disturb = toggle(
        config,
        match (notif_state.dnd, any_pending) {
            (true, _) => Solid::CommentSlash.into(),
            (false, true) => Solid::Comment.into(),
            (false, false) => Regular::Comment.into(),
        },
        !notif_state.dnd,
        NotificationGroupMessage::DoNotDisturb,
    );
    let sound = toggle(
        config,
        match (notif_state.sound, any_pending) {
            (true, true) => Solid::Bell.into(),
            (true, false) => Regular::Bell.into(),
            (false, true) => Solid::BellSlash.into(),
            (false, false) => Regular::BellSlash.into(),
        },
        notif_state.sound,
        NotificationGroupMessage::Sound,
    );

    let spatial_layout = toggle(
        config,
        if notif_state.spatial_layout {
            Solid::AlignCenter.into()
        } else {
            Solid::AlignRight.into()
        },
        notif_state.spatial_layout,
        NotificationGroupMessage::SpatialLayout,
    );

    let toggles = Row::new()
        .push(do_not_disturb)
        .push(sound)
        .push(spatial_layout)
        .spacing(config.spacing.widget_spacing_px)
        .align_y(Alignment::Center);
    permanently_inactive_slot(toggles.into(), config, position)
}

fn toggle<'a>(
    config: &Config,
    icon: FontAwesome,
    active: bool,
    message: NotificationGroupMessage,
) -> Element<'a, NotificationGroupMessage> {
    let opacity = if active { 1.0 } else { DISABLED_ALPHA };
    let color = config.palette.bar_foreground.scale_alpha(opacity);
    button(icon.glyph().size(config.font.icon_size_px).color(color))
        .padding(0)
        .style(flat_button(color))
        .on_press(message)
        .into()
}
