use iced_widget::core::{Color, Theme};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    pub line: Color,
    pub anchor: Color,
}

pub type StyleFn<'a> = Box<dyn Fn(&Theme) -> Style + 'a>;

#[must_use]
pub fn default(theme: &Theme) -> Style {
    let palette = theme.extended_palette();
    Style {
        line: palette.background.strong.color,
        anchor: palette.primary.base.color,
    }
}
