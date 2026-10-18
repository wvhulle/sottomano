use iced::{
    Color, Element, Theme,
    widget::rule::{self, FillMode},
};
use vetrata_config::screen_edges::Position;

#[must_use]
pub fn activity_line<'a, Message: 'a>(
    color: Color,
    opacity: f32,
    thickness: f32,
) -> Element<'a, Message> {
    rule::horizontal(thickness)
        .style(line_style(color, opacity, thickness))
        .into()
}

#[must_use]
pub fn edge_activity_line<'a, Message: 'a>(
    edge: Position,
    color: Color,
    opacity: f32,
    thickness: f32,
) -> Element<'a, Message> {
    if edge.is_vertical() {
        rule::vertical(thickness)
            .style(line_style(color, opacity, thickness))
            .into()
    } else {
        activity_line(color, opacity, thickness)
    }
}

fn line_style(color: Color, opacity: f32, thickness: f32) -> impl Fn(&Theme) -> rule::Style {
    move |_| rule::Style {
        color: color.scale_alpha(opacity),
        radius: thickness.into(),
        fill_mode: FillMode::Full,
        snap: true,
    }
}
