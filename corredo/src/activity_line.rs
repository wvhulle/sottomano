use iced::{
    Color, Element, Theme,
    widget::rule::{self, FillMode},
};

#[must_use]
pub fn activity_line<'a, Message: 'a>(
    color: impl Fn(&Theme) -> Color + 'a,
    opacity: f32,
    thickness: f32,
) -> Element<'a, Message> {
    rule::horizontal(thickness)
        .style(line_style(color, opacity, thickness))
        .into()
}

#[must_use]
pub fn vertical_activity_line<'a, Message: 'a>(
    color: impl Fn(&Theme) -> Color + 'a,
    opacity: f32,
    thickness: f32,
) -> Element<'a, Message> {
    rule::vertical(thickness)
        .style(line_style(color, opacity, thickness))
        .into()
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
