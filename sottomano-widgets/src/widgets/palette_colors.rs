use iced::{Color, Theme, theme};
use sottomano_config::theme::palette::Palette;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaletteColors {
    pub surface: Color,
    pub bar_foreground: Color,
    pub inactive: Color,
    pub urgent: Color,
    pub warning: Color,
    pub focus: Color,
    pub success: Color,
    pub info: Color,
    pub dim: Color,
    pub surface_raised: Color,
    pub frame_outline: Color,
    pub secondary_foreground: Color,
}

impl From<&Palette> for PaletteColors {
    fn from(palette: &Palette) -> Self {
        Self {
            surface: Color::from(palette.surface.components),
            bar_foreground: Color::from(palette.bar_foreground.components),
            inactive: Color::from(palette.inactive.components),
            urgent: Color::from(palette.urgent.components),
            warning: Color::from(palette.warning.components),
            focus: Color::from(palette.focus.components),
            success: Color::from(palette.success.components),
            info: Color::from(palette.info.components),
            dim: Color::from(palette.dim.components),
            surface_raised: Color::from(palette.surface_raised.components),
            frame_outline: Color::from(palette.frame_outline.components),
            secondary_foreground: Color::from(palette.secondary_foreground.components),
        }
    }
}

impl From<&PaletteColors> for Theme {
    fn from(colors: &PaletteColors) -> Self {
        Theme::custom(
            String::from("umbra"),
            theme::Palette {
                background: colors.surface,
                text: colors.bar_foreground,
                primary: colors.focus,
                success: colors.success,
                warning: colors.warning,
                danger: colors.urgent,
            },
        )
    }
}
