use iced_widget::{
    canvas::{self, Frame, Geometry, Path, Stroke},
    core::{Rectangle, Theme, mouse},
};

use crate::{callout_layout::LeaderLine, diagram_style::StyleFn};

const LINE_WIDTH: f32 = 1.0;
const ANCHOR_RADIUS: f32 = 2.5;

pub(crate) struct LeaderLines<'a> {
    pub leaders: Vec<LeaderLine>,
    pub style: StyleFn<'a>,
}

impl<Message> canvas::Program<Message> for LeaderLines<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced_widget::Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let style = (self.style)(theme);
        let mut frame = Frame::new(renderer, bounds.size());
        let routes = Path::new(|path| {
            for leader in &self.leaders {
                path.move_to(leader.label_end);
                path.line_to(leader.bend);
                path.line_to(leader.anchor);
            }
        });
        frame.stroke(
            &routes,
            Stroke::default()
                .with_width(LINE_WIDTH)
                .with_color(style.line),
        );
        for leader in &self.leaders {
            frame.fill(&Path::circle(leader.anchor, ANCHOR_RADIUS), style.anchor);
        }
        vec![frame.into_geometry()]
    }
}
