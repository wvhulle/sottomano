use iced_widget::{
    Renderer, canvas, column, container,
    core::{Alignment, Element, Length, Point, Theme},
    image, pin, row, space, stack,
};

use crate::{
    callout_layout::{CalloutLayout, CalloutSide, DiagramMetrics, callout_layout},
    diagram_anchor::DiagramAnchor,
    diagram_style::{Style, StyleFn, default},
    face_style::FaceStyle,
    leader_lines::LeaderLines,
    sdl_gamepad_artwork::{ControllerArtwork, FACE_OFFSET, FACE_SIZE},
};

struct Callout<'a, Message> {
    anchor: DiagramAnchor,
    label: Element<'a, Message, Theme, Renderer>,
}

pub struct ControllerDiagram<'a, Message> {
    artwork: ControllerArtwork,
    face: FaceStyle,
    metrics: DiagramMetrics,
    callouts: Vec<Callout<'a, Message>>,
    style: StyleFn<'a>,
}

#[must_use]
pub fn controller_diagram<'a, Message: 'a>(
    artwork: ControllerArtwork,
    face: FaceStyle,
) -> ControllerDiagram<'a, Message> {
    ControllerDiagram::new(artwork, face)
}

impl<'a, Message: 'a> ControllerDiagram<'a, Message> {
    #[must_use]
    pub fn new(artwork: ControllerArtwork, face: FaceStyle) -> Self {
        Self {
            artwork,
            face,
            metrics: DiagramMetrics {
                scale: 0.5,
                row_height: 18.0,
                row_gap: 2.0,
                gutter: 28.0,
                stub: 8.0,
            },
            callouts: Vec::new(),
            style: Box::new(default),
        }
    }

    #[must_use]
    pub fn scale(mut self, scale: f32) -> Self {
        self.metrics.scale = scale;
        self
    }

    #[must_use]
    pub fn row_height(mut self, row_height: f32) -> Self {
        self.metrics.row_height = row_height;
        self
    }

    #[must_use]
    pub fn spacing(mut self, row_gap: f32) -> Self {
        self.metrics.row_gap = row_gap;
        self
    }

    #[must_use]
    pub fn gutter(mut self, gutter: f32) -> Self {
        self.metrics.gutter = gutter;
        self
    }

    #[must_use]
    pub fn callout(
        mut self,
        anchor: impl Into<DiagramAnchor>,
        label: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        self.callouts.push(Callout {
            anchor: anchor.into(),
            label: label.into(),
        });
        self
    }

    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self {
        self.style = Box::new(style);
        self
    }

    fn label_column(
        rows: Vec<(f32, Element<'a, Message, Theme, Renderer>)>,
        metrics: DiagramMetrics,
        align: Alignment,
    ) -> Element<'a, Message, Theme, Renderer> {
        let mut next_free = 0.0;
        column(rows.into_iter().flat_map(|(top, label)| {
            let gap = (top - next_free).max(0.0);
            next_free = top + metrics.row_height;
            [
                space().height(gap).into(),
                container(label)
                    .height(metrics.row_height)
                    .center_y(metrics.row_height)
                    .into(),
            ]
        }))
        .align_x(align)
        .into()
    }

    fn artwork(&self, layout: &CalloutLayout) -> Vec<Element<'a, Message, Theme, Renderer>> {
        let scale = self.metrics.scale;
        let body = layout.body;
        let body_art = pin(image(self.artwork.body().clone())
            .width(body.width)
            .height(body.height))
        .position(body.position());
        let face_art = self.artwork.face(self.face).map(|face| {
            pin(image(face.clone())
                .width(FACE_SIZE.width * scale)
                .height(FACE_SIZE.height * scale))
            .position(Point::new(
                body.x + FACE_OFFSET.x * scale,
                body.y + FACE_OFFSET.y * scale,
            ))
            .into()
        });
        [body_art.into()].into_iter().chain(face_art).collect()
    }
}

impl<'a, Message: 'a> From<ControllerDiagram<'a, Message>>
    for Element<'a, Message, Theme, Renderer>
{
    fn from(diagram: ControllerDiagram<'a, Message>) -> Self {
        let anchors: Vec<DiagramAnchor> = diagram
            .callouts
            .iter()
            .map(|callout| callout.anchor)
            .collect();
        let layout = callout_layout(&anchors, diagram.metrics);
        let artwork = diagram.artwork(&layout);
        let metrics = diagram.metrics;
        let (mut left, mut right): (Vec<_>, Vec<_>) = diagram
            .callouts
            .into_iter()
            .zip(&layout.tops)
            .map(|(callout, &top)| (CalloutSide::of(callout.anchor), top, callout.label))
            .partition(|(side, _, _)| *side == CalloutSide::Left);
        let by_top = |a: &(CalloutSide, f32, _), b: &(CalloutSide, f32, _)| a.1.total_cmp(&b.1);
        left.sort_by(by_top);
        right.sort_by(by_top);
        let rows = |side: Vec<(CalloutSide, f32, Element<'a, Message, Theme, Renderer>)>| {
            side.into_iter()
                .map(|(_, top, label)| (top, label))
                .collect()
        };
        let lines = canvas(LeaderLines {
            leaders: layout.leaders,
            style: diagram.style,
        })
        .width(layout.art_size.width)
        .height(layout.art_size.height);
        let art = stack(artwork.into_iter().chain([lines.into()]))
            .width(layout.art_size.width)
            .height(layout.art_size.height);
        row![
            ControllerDiagram::label_column(rows(left), metrics, Alignment::End),
            art,
            ControllerDiagram::label_column(rows(right), metrics, Alignment::Start),
        ]
        .height(Length::Shrink)
        .into()
    }
}
