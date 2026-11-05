use iced::{
    Color, Theme,
    theme::{
        self,
        palette::{Extended, Pair},
    },
};
use sottomano_toml_config::theme::palette::Palette;

const THEME_NAME: &str = "sottomano";

pub type ThemedColor<'a> = Box<dyn Fn(&Theme) -> Color + 'a>;

#[must_use]
pub fn configured_theme(palette: &Palette) -> Theme {
    let text = Color::from(palette.text.components);
    let background_weak = Color::from(palette.background_weak.components);
    let background_strong = Color::from(palette.background_strong.components);
    let secondary = Color::from(palette.secondary.components);
    Theme::custom_with_fn(
        THEME_NAME,
        theme::Palette {
            background: Color::from(palette.background.components),
            text,
            primary: Color::from(palette.primary.components),
            success: Color::from(palette.success.components),
            warning: Color::from(palette.warning.components),
            danger: Color::from(palette.danger.components),
        },
        move |base| {
            let mut extended = Extended::generate(base);
            extended.background.weak = Pair::new(background_weak, text);
            extended.background.strong = Pair::new(background_strong, text);
            extended.secondary.base = Pair::new(secondary, base.background);
            extended
        },
    )
}

#[must_use]
pub fn text_color(theme: &Theme) -> Color {
    theme.palette().text
}

#[must_use]
pub fn secondary_text_color(theme: &Theme) -> Color {
    theme.extended_palette().secondary.base.color
}

#[must_use]
pub fn background_color(theme: &Theme) -> Color {
    theme.palette().background
}

#[must_use]
pub fn background_weak_color(theme: &Theme) -> Color {
    theme.extended_palette().background.weak.color
}

#[must_use]
pub fn background_strong_color(theme: &Theme) -> Color {
    theme.extended_palette().background.strong.color
}

#[must_use]
pub fn primary_color(theme: &Theme) -> Color {
    theme.palette().primary
}

#[must_use]
pub fn primary_weak_color(theme: &Theme) -> Color {
    theme.extended_palette().primary.weak.color
}

#[must_use]
pub fn success_color(theme: &Theme) -> Color {
    theme.palette().success
}

#[must_use]
pub fn warning_color(theme: &Theme) -> Color {
    theme.palette().warning
}

#[must_use]
pub fn danger_color(theme: &Theme) -> Color {
    theme.palette().danger
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AccentShades {
    pub base: Color,
    pub weak: Color,
}

#[must_use]
pub fn primary_shades(theme: &Theme) -> AccentShades {
    let primary = theme.extended_palette().primary;
    AccentShades {
        base: primary.base.color,
        weak: primary.weak.color,
    }
}

#[must_use]
pub fn success_shades(theme: &Theme) -> AccentShades {
    let success = theme.extended_palette().success;
    AccentShades {
        base: success.base.color,
        weak: success.weak.color,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_colours_land_in_the_extended_palette() {
        let palette = Palette::default();
        let theme = configured_theme(&palette);
        assert_eq!(
            background_weak_color(&theme),
            Color::from(palette.background_weak.components)
        );
        assert_eq!(
            background_strong_color(&theme),
            Color::from(palette.background_strong.components)
        );
        assert_eq!(
            secondary_text_color(&theme),
            Color::from(palette.secondary.components)
        );
        assert_eq!(text_color(&theme), Color::from(palette.text.components));
        assert_eq!(
            primary_color(&theme),
            Color::from(palette.primary.components)
        );
    }
}
