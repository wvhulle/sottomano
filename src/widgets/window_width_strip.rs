use iced::{
    Border, Color, Element, Length, Theme,
    widget::{Row, Space, container},
};

pub struct WidthSegment {
    pub width: f32,
    pub color: Color,
}

pub struct WindowWidthStrip {
    segments: Vec<WidthSegment>,
    thickness: f32,
}

impl WindowWidthStrip {
    pub fn new(segments: Vec<WidthSegment>, thickness: f32) -> Self {
        Self {
            segments,
            thickness,
        }
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn width_portion(width: f32) -> u16 {
    width.round().clamp(1.0, f32::from(u16::MAX)) as u16
}

fn rounded_fill(color: Color, thickness: f32) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(color.into()),
        border: Border::default().rounded(thickness / 2.0),
        ..container::Style::default()
    }
}

impl<'a, Message: 'a> From<WindowWidthStrip> for Element<'a, Message> {
    fn from(strip: WindowWidthStrip) -> Self {
        let thickness = strip.thickness;
        Row::with_children(strip.segments.into_iter().map(|segment| {
            container(Space::new())
                .width(Length::FillPortion(width_portion(segment.width)))
                .height(thickness)
                .style(rounded_fill(segment.color, thickness))
                .into()
        }))
        .spacing(thickness)
        .width(Length::Fill)
        .height(thickness)
        .into()
    }
}
