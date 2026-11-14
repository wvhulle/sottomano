use iced::{
    Color, Element, Theme,
    widget::rule::{self, FillMode},
};

use crate::orientation::Axis;

#[must_use]
pub fn activity_line<'a, Message: 'a>(
    axis: Axis,
    color: impl Fn(&Theme) -> Color + 'a,
    opacity: f32,
    thickness: f32,
) -> Element<'a, Message> {
    let line_style = line_style(color, opacity, thickness);
    match axis {
        Axis::Horizontal => rule::horizontal(thickness).style(line_style).into(),
        Axis::Vertical => rule::vertical(thickness).style(line_style).into(),
    }
}

fn line_style<'a>(
    color: impl Fn(&Theme) -> Color + 'a,
    opacity: f32,
    thickness: f32,
) -> impl Fn(&Theme) -> rule::Style + 'a {
    move |theme| rule::Style {
        color: color(theme).scale_alpha(opacity),
        radius: thickness.into(),
        fill_mode: FillMode::Full,
        snap: true,
    }
}
