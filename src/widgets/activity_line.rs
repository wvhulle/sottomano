use iced::{
    Border, Color, Element, Length,
    widget::{Space, container},
};

pub fn activity_line<'a, Message: 'a>(
    color: Color,
    opacity: f32,
    thickness: f32,
) -> Element<'a, Message> {
    container(Space::new().width(Length::Fill))
        .width(Length::Fill)
        .height(thickness)
        .style(move |_| container::Style {
            background: Some(
                Color {
                    a: opacity,
                    ..color
                }
                .into(),
            ),
            border: Border::default().rounded(thickness),
            ..Default::default()
        })
        .into()
}
