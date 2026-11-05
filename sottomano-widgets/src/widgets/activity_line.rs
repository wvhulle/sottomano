pub use corredo::activity_line::activity_line;
use corredo::activity_line::vertical_activity_line;
use iced::{Color, Element, Theme};
use sottomano_toml_config::screen_edges::Position;

#[must_use]
pub fn edge_activity_line<'a, Message: 'a>(
    edge: Position,
    color: impl Fn(&Theme) -> Color + 'a,
    opacity: f32,
    thickness: f32,
) -> Element<'a, Message> {
    if edge.is_vertical() {
        vertical_activity_line(color, opacity, thickness)
    } else {
        activity_line(color, opacity, thickness)
    }
}
