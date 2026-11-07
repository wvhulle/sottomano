use corredo::confirm_button::ConfirmButtonIcons;
pub use corredo::confirm_button::{ConfirmButton, ConfirmPress};
use iced::{Element, Theme};
use sottomano_toml_config::Config;

use crate::{
    group_items::reading_display_state::{ReadingDisplayState, ReadingStateTint},
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
};

#[allow(clippy::too_many_arguments)]
pub fn configured_confirm_button<'a, Message: 'a + Clone>(
    button: &ConfirmButton,
    config: &Config,
    opacity: f32,
    idle_glyph: Phosphor,
    on_press: Message,
    on_cancel: Message,
) -> Element<'a, Message> {
    let size = config.theme.font.icon_size_px;
    let tinted_icon = |glyph: Phosphor, state: ReadingDisplayState| {
        let tint = ReadingStateTint { state, opacity };
        IconGlyph::from(glyph).view(size, move |theme: &Theme| tint.color(theme))
    };
    let idle_state = ReadingDisplayState::Off;
    button.view(
        ConfirmButtonIcons {
            idle: tinted_icon(idle_glyph.with_weight(idle_state.weight()), idle_state),
            confirm: tinted_icon(Phosphor::Check(Weight::Bold), ReadingDisplayState::Pending),
            cancel: tinted_icon(Phosphor::X(Weight::Regular), ReadingDisplayState::Off),
        },
        config.theme.spacing.item_gap,
        on_press,
        on_cancel,
    )
}
