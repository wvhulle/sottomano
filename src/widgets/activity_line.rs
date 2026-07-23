use iced::{Border, Color, Element, Length, widget::container};

/// odin's `Hairline`: a thin rounded bar, the shared state-indicator primitive
/// for the menu-group expand affordance, the pager focus/urgent bars, and the
/// per-window tick segments — each just picks a `color`, `opacity`, and
/// `thickness`. Fill-width (the caller's slot sizes it); the caller pins it to
/// an edge via `pager_slot`.
pub fn activity_line<'a, Message: 'a>(color: Color, opacity: f32, thickness: f32) -> Element<'a, Message> {
    container(iced::widget::Space::new().width(Length::Fill))
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
