//! The notification controls: two always-visible bar toggles for do-not-disturb
//! (suppress and discard toasts) and the alert sound. Stateless — the truth
//! lives in [`ResolvedConfig::dnd`]/[`ResolvedConfig::sound`], which the
//! application owns and applies; this only renders it and emits the press
//! intents.

use iced::{
    Alignment, Element,
    widget::{Row, mouse_area},
};

use crate::{config::resolved::ResolvedConfig, widgets::icon::Icon};

#[derive(Debug, Clone, Copy)]
pub enum NotificationGroupMessage {
    ToggleDoNotDisturb,
    ToggleSound,
}

pub fn view<'a>(config: &ResolvedConfig) -> Element<'a, NotificationGroupMessage> {
    let do_not_disturb = toggle(
        config,
        if config.dnd {
            Icon::BellSlash
        } else {
            Icon::Bell
        },
        // Muted reads as inactive, so dim it.
        !config.dnd,
        NotificationGroupMessage::ToggleDoNotDisturb,
    );
    let sound = toggle(
        config,
        if config.sound {
            Icon::VolumeHigh
        } else {
            Icon::VolumeXmark
        },
        config.sound,
        NotificationGroupMessage::ToggleSound,
    );

    Row::new()
        .push(do_not_disturb)
        .push(sound)
        .spacing(config.layout.horizontal_widget_spacing_px)
        .align_y(Alignment::Center)
        .into()
}

/// One toggle icon, full-strength when `active` and dimmed when not.
fn toggle<'a>(
    config: &ResolvedConfig,
    icon: Icon,
    active: bool,
    message: NotificationGroupMessage,
) -> Element<'a, NotificationGroupMessage> {
    let color = config
        .palette
        .bar_foreground
        .scale_alpha(if active { 1.0 } else { 0.4 });
    mouse_area(icon.solid().size(config.font.icon_size_px).color(color))
        .on_press(message)
        .into()
}
