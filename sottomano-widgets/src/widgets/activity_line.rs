use iced::{
    Color, Element, Theme,
    widget::rule::{self, FillMode},
};
use sottomano_toml_config::screen_edges::Position;

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
pub fn edge_activity_line<'a, Message: 'a>(
    edge: Position,
    color: impl Fn(&Theme) -> Color + 'a,
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
