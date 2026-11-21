use corredo::press_actions::{ConfirmButton, ConfirmButtonIcons};
use iced::{Element, Theme};
use sottomano_toml_config::Config;

use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::indicator_state::{IndicatorState, IndicatorStateTint},
};

#[allow(clippy::too_many_arguments)]
pub fn configured_confirm_button<'view, Message: 'view + Clone>(
    button: &ConfirmButton,
    config: &Config,
    opacity: f32,
    idle_glyph: Phosphor,
    on_press: Message,
    on_cancel: Message,
) -> Element<'view, Message> {
    let size = config.theme.font.icon_size_px;
    let tinted_icon = |glyph: Phosphor, state: IndicatorState| {
        let tint = IndicatorStateTint { state, opacity };
        IconGlyph::from(glyph).view(size, move |theme: &Theme| tint.color(theme))
    };
    let idle_state = IndicatorState::Off;
    button.view(
        ConfirmButtonIcons {
            idle: tinted_icon(idle_glyph.with_weight(idle_state.weight()), idle_state),
            confirm: tinted_icon(Phosphor::Check(Weight::Bold), IndicatorState::Pending),
            cancel: tinted_icon(Phosphor::X(Weight::Regular), IndicatorState::Off),
        },
        config.theme.spacing.item_gap,
        on_press,
        on_cancel,
    )
}
