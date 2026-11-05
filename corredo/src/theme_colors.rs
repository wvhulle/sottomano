use iced::{Color, Theme};

pub type ThemedColor<'a> = Box<dyn Fn(&Theme) -> Color + 'a>;

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
