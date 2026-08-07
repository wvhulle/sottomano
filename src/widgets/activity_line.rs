use iced::{
    Color, Element,
    widget::rule::{self, FillMode},
};

pub fn activity_line<'a, Message: 'a>(
    color: Color,
    opacity: f32,
    thickness: f32,
) -> Element<'a, Message> {
    rule::horizontal(thickness)
        .style(move |_| rule::Style {
            color: color.scale_alpha(opacity),
            radius: thickness.into(),
            fill_mode: FillMode::Full,
            snap: true,
        })
        .into()
}
