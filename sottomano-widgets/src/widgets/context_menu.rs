pub use corredo::context_menu::{ContextMenuLine, ContextMenuRow, ContextMenuRowPress, Trailing};
use corredo::context_menu::{ContextMenuMetrics, context_menu as themed_context_menu};
use iced::{Color, Element, Theme, border::Radius};
use sottomano_toml_config::Config;

use crate::widgets::{
    icon_glyph::IconGlyph,
    phosphor::{Phosphor, Weight},
    text::text_face_of,
};

pub fn context_menu<'a, M: Clone + 'a>(
    config: &Config,
    popup_corners: Radius,
    lines: impl IntoIterator<Item = ContextMenuLine<'a, M>>,
) -> Element<'a, M> {
    let icon_size_px = config.theme.font.icon_size_px;
    themed_context_menu(lines)
        .corners(popup_corners)
        .metrics(ContextMenuMetrics {
            face: text_face_of(&config.theme.font),
            note_size_px: config.theme.font.small_text_size_px,
            icon_size_px,
            item_gap_px: config.theme.spacing.item_gap,
            surface_inset_px: config.theme.spacing.surface_inset,
        })
        .submenu_chevron(move |open: bool, color: fn(&Theme) -> Color| {
            let chevron = if open {
                Phosphor::CaretDown(Weight::Regular)
            } else {
                Phosphor::CaretRight(Weight::Regular)
            };
            IconGlyph::from(chevron).view(icon_size_px, color)
        })
        .into()
}
