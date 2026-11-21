use std::ops::{Range, RangeInclusive};

use iced::{
    Color, Event, Length, Point, Rectangle, Size, Theme,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        renderer,
        widget::{Tree, tree},
    },
    mouse::{self, Cursor},
};

use super::SliderAccents;
use crate::{
    pointer_hover::{PointerHover, hover_over_bounds},
    quad_shapes::{CircleQuad, HorizontalCapsuleQuad, QuadOutline},
    theming::{background_color, background_strong_color},
    value_controls::SourceLag,
};

const RESTING_KNOB_PER_GLYPH: f32 = 0.17;
const ENGAGED_KNOB_GROWTH: f32 = 1.25;

#[derive(Debug, Clone, Copy)]
pub(super) struct FaceGeometry {
    pub(super) stroke_width: f32,
    resting_knob_radius: f32,
    engaged_knob_radius: f32,
}

impl FaceGeometry {
    pub(super) fn scaled_to_icon(icon_size_px: f32, stroke_width: f32) -> Self {
        let resting_knob_radius = icon_size_px * RESTING_KNOB_PER_GLYPH;
        Self {
            stroke_width,
            resting_knob_radius,
            engaged_knob_radius: resting_knob_radius * ENGAGED_KNOB_GROWTH,
        }
    }

    pub(super) fn edge_inset(self) -> f32 {
        self.engaged_knob_radius + self.stroke_width / 2.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KnobEngagement {
    Engaged,
    Resting,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum PointerEngagement {
    #[default]
    Away,
    Hovering,
    Dragging,
}

impl PointerEngagement {
    fn after_event(self, event: &Event, bounds: Rectangle, cursor: Cursor) -> Self {
        let pointer_hover = hover_over_bounds(cursor, bounds);
        let hover_engagement = match pointer_hover {
            PointerHover::Over => Self::Hovering,
            PointerHover::Outside => Self::Away,
        };
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => match pointer_hover {
                PointerHover::Over => Self::Dragging,
                PointerHover::Outside => Self::Away,
            },
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => hover_engagement,
            Event::Mouse(_) => match self {
                Self::Dragging => Self::Dragging,
                Self::Away | Self::Hovering => hover_engagement,
            },
            _ => self,
        }
    }

    fn knob_engagement(self) -> KnobEngagement {
        match self {
            Self::Hovering | Self::Dragging => KnobEngagement::Engaged,
            Self::Away => KnobEngagement::Resting,
        }
    }
}

pub(super) struct SliderFace<'view> {
    pub(super) range: RangeInclusive<f32>,
    pub(super) value: f32,
    pub(super) source_lag: SourceLag,
    pub(super) accents: SliderAccents<'view>,
    pub(super) geometry: FaceGeometry,
    pub(super) face_size: Size,
}

impl<Message> Widget<Message, Theme, iced::Renderer> for SliderFace<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<PointerEngagement>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(PointerEngagement::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(
            Length::Fixed(self.face_size.width),
            Length::Fixed(self.face_size.height),
        )
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &iced::Renderer, limits: &Limits) -> Node {
        Node::new(limits.resolve(self.face_size.width, self.face_size.height, Size::ZERO))
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let engagement = tree.state.downcast_mut::<PointerEngagement>();
        let next_engagement = engagement.after_event(event, layout.bounds(), cursor);
        if next_engagement != *engagement {
            *engagement = next_engagement;
            shell.request_redraw();
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: Cursor,
        _viewport: &Rectangle,
    ) {
        let engagement = tree.state.downcast_ref::<PointerEngagement>();
        let bounds = layout.bounds();
        let geometry = self.geometry;
        let (range_start, range_end) = (*self.range.start(), *self.range.end());
        let span = range_end - range_start;
        let edge_inset = geometry.edge_inset();
        let centre_of = |position: f32| {
            let fraction = if span > 0.0 {
                ((position - range_start) / span).clamp(0.0, 1.0)
            } else {
                0.0
            };
            bounds.x + (bounds.width - 2.0 * edge_inset) * fraction + edge_inset
        };

        let knob_engagement = match self.source_lag {
            SourceLag::LaggingAt(_) => KnobEngagement::Engaged,
            SourceLag::CaughtUp => engagement.knob_engagement(),
        };
        let (accent, knob_radius) = match knob_engagement {
            KnobEngagement::Engaged => {
                ((self.accents.engaged)(theme), geometry.engaged_knob_radius)
            }
            KnobEngagement::Resting => {
                ((self.accents.resting)(theme), geometry.resting_knob_radius)
            }
        };

        let rail_y = bounds.y + bounds.height / 2.0;
        let handle_x = centre_of(self.value);
        let lagging_gap = match self.source_lag {
            SourceLag::LaggingAt(lagging) => {
                let lagging_x = centre_of(lagging);
                lagging_x - geometry.resting_knob_radius..lagging_x + geometry.resting_knob_radius
            }
            SourceLag::CaughtUp => f32::INFINITY..f32::INFINITY,
        };
        let accent_outline = QuadOutline {
            color: accent,
            width: geometry.stroke_width,
        };

        [
            (
                bounds.x + edge_inset,
                bounds.x + bounds.width - edge_inset,
                background_strong_color(theme),
            ),
            (bounds.x + edge_inset, handle_x, accent),
        ]
        .into_iter()
        .flat_map(|(segment_start, segment_end, color)| {
            without_gap(segment_start, segment_end, lagging_gap.clone())
                .map(move |(piece_start, piece_end)| (piece_start, piece_end, color))
        })
        .for_each(|(piece_start, piece_end, color)| {
            HorizontalCapsuleQuad {
                start_x: piece_start,
                end_x: piece_end,
                centre_y: rail_y,
                thickness: geometry.stroke_width,
            }
            .draw(renderer, color);
        });

        if let SourceLag::LaggingAt(lagging_value) = self.source_lag {
            CircleQuad::stroked_path(
                Point::new(centre_of(lagging_value), rail_y),
                geometry.resting_knob_radius,
                geometry.stroke_width,
            )
            .draw(renderer, Color::TRANSPARENT, accent_outline);
        }

        CircleQuad::stroked_path(
            Point::new(handle_x, rail_y),
            knob_radius,
            geometry.stroke_width,
        )
        .draw(renderer, background_color(theme), accent_outline);
    }
}

fn without_gap(
    segment_start: f32,
    segment_end: f32,
    gap: Range<f32>,
) -> impl Iterator<Item = (f32, f32)> {
    [
        (segment_start, segment_end.min(gap.start)),
        (segment_start.max(gap.end), segment_end),
    ]
    .into_iter()
    .filter(|(piece_start, piece_end)| piece_end > piece_start)
}
