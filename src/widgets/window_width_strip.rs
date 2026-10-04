use iced::{
    Border, Color, Element, Theme,
    widget::{Row, Space, container},
};

pub struct WidthSegment {
    pub width_px: f32,
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
                .width(segment.width_px.max(thickness))
                .height(thickness)
                .style(rounded_fill(segment.color, thickness))
                .into()
        }))
        .spacing(thickness)
        .height(thickness)
        .into()
    }
}
