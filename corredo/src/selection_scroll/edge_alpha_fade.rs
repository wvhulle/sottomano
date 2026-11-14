use std::f32::consts::{FRAC_PI_2, PI};

use iced::{
    Border, Color, Radians, Rectangle, Theme,
    advanced::{Renderer as _, renderer},
    border::Radius,
    gradient::Linear,
};
use iced_wgpu::primitive::Renderer as _;

use crate::{
    alpha_fade_shaders::{FadeShader, ReplacingFade},
    orientation::{Axis, Edge},
    theming::FrameStyle,
};

const OVERFLOW_TOLERANCE_PX: f32 = 0.5;
const RADIUS_FRACTION_SCALE: f32 = 65535.0;
const EDGE_INDEX_BITS: u32 = 2;

impl Edge {
    fn shader_index(self) -> u32 {
        match self {
            Self::Left => 0,
            Self::Right => 1,
            Self::Top => 2,
            Self::Bottom => 3,
        }
    }

    fn packed_with_radius_fraction(self, radius_fraction: f32) -> u32 {
        let scaled_fraction = (radius_fraction.clamp(0.0, 1.0) * RADIUS_FRACTION_SCALE).round();
        self.shader_index() | ((scaled_fraction as u32) << EDGE_INDEX_BITS)
    }

    fn along_extent(self, band: Rectangle) -> f32 {
        match self {
            Self::Left | Self::Right => band.width,
            Self::Top | Self::Bottom => band.height,
        }
    }

    fn gradient_direction(self) -> Radians {
        match self {
            Self::Top => Radians(0.0),
            Self::Right => Radians(FRAC_PI_2),
            Self::Bottom => Radians(PI),
            Self::Left => Radians(PI + FRAC_PI_2),
        }
    }

    fn rounding_only_outer_corners(self, radius_px: f32) -> Radius {
        let rounded = Radius::from(radius_px);
        match self {
            Self::Top => Radius {
                bottom_left: 0.0,
                bottom_right: 0.0,
                ..rounded
            },
            Self::Bottom => Radius {
                top_left: 0.0,
                top_right: 0.0,
                ..rounded
            },
            Self::Left => Radius {
                top_right: 0.0,
                bottom_right: 0.0,
                ..rounded
            },
            Self::Right => Radius {
                top_left: 0.0,
                bottom_left: 0.0,
                ..rounded
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ScrollEdgeFade {
    frame: FrameStyle,
    extent_px: f32,
    corner_radius_px: f32,
    outline_inset_px: f32,
}

impl ScrollEdgeFade {
    #[must_use]
    pub fn within_frame(frame: FrameStyle, extent_px: f32) -> Self {
        let outline_inset_px = frame.outline_width();
        Self {
            frame,
            extent_px,
            corner_radius_px: (frame.corner_radius().top_left - outline_inset_px).max(0.0),
            outline_inset_px,
        }
    }

    #[must_use]
    pub fn away_from_frame(frame: FrameStyle, extent_px: f32) -> Self {
        Self {
            frame,
            extent_px,
            corner_radius_px: 0.0,
            outline_inset_px: 0.0,
        }
    }

    #[must_use]
    pub fn extent_within(self, viewport_span: f32) -> f32 {
        self.extent_px.min(viewport_span / 2.0)
    }

    pub(crate) fn clear_of_outline(
        self,
        clip: Rectangle,
        axis: Axis,
        hidden_edges: HiddenScrollEdges,
    ) -> Rectangle {
        let inset = |hidden: bool| if hidden { self.outline_inset_px } else { 0.0 };
        let start_inset = inset(hidden_edges.start);
        let end_inset = inset(hidden_edges.end);
        match axis {
            Axis::Horizontal => Rectangle {
                x: clip.x + start_inset,
                width: (clip.width - start_inset - end_inset).max(0.0),
                ..clip
            },
            Axis::Vertical => Rectangle {
                y: clip.y + start_inset,
                height: (clip.height - start_inset - end_inset).max(0.0),
                ..clip
            },
        }
    }

    fn band_along(self, faded_edge: Edge, bounds: Rectangle) -> Rectangle {
        let inner = Rectangle {
            x: bounds.x + self.outline_inset_px,
            y: bounds.y + self.outline_inset_px,
            width: bounds.width - 2.0 * self.outline_inset_px,
            height: bounds.height - 2.0 * self.outline_inset_px,
        };
        match faded_edge {
            Edge::Left => Rectangle {
                width: self.extent_within(inner.width),
                ..inner
            },
            Edge::Right => {
                let width = self.extent_within(inner.width);
                Rectangle {
                    x: inner.x + inner.width - width,
                    width,
                    ..inner
                }
            }
            Edge::Top => Rectangle {
                height: self.extent_within(inner.height),
                ..inner
            },
            Edge::Bottom => {
                let height = self.extent_within(inner.height);
                Rectangle {
                    y: inner.y + inner.height - height,
                    height,
                    ..inner
                }
            }
        }
    }

    pub fn draw_on(
        self,
        faded_edge: Edge,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        bounds: Rectangle,
    ) {
        let band = self.band_along(faded_edge, bounds);
        let restored_fill = self.frame.fill_color(theme);
        if matches!(renderer, iced::Renderer::Primary(_)) {
            renderer.draw_primitive(
                band,
                ReplacingFade::<EdgeFadeShader>::new(
                    faded_edge.packed_with_radius_fraction(
                        self.corner_radius_px / faded_edge.along_extent(band),
                    ),
                    restored_fill,
                ),
            );
        } else {
            self.draw_gradient_band(faded_edge, renderer, band, restored_fill);
        }
    }

    fn draw_gradient_band(
        self,
        faded_edge: Edge,
        renderer: &mut iced::Renderer,
        band: Rectangle,
        solid_fill: Color,
    ) {
        renderer.fill_quad(
            renderer::Quad {
                bounds: band,
                border: Border {
                    radius: faded_edge.rounding_only_outer_corners(self.corner_radius_px),
                    ..Border::default()
                },
                ..renderer::Quad::default()
            },
            Linear::new(faded_edge.gradient_direction())
                .add_stop(
                    0.0,
                    Color {
                        a: 0.0,
                        ..solid_fill
                    },
                )
                .add_stop(1.0, solid_fill),
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HiddenScrollEdges {
    pub start: bool,
    pub end: bool,
}

impl HiddenScrollEdges {
    #[must_use]
    pub fn of(scrolled_px: f32, content_span: f32, visible_span: f32) -> Self {
        let max_scrolled = (content_span - visible_span).max(0.0);
        let scrolled = scrolled_px.clamp(0.0, max_scrolled);
        Self {
            start: scrolled > OVERFLOW_TOLERANCE_PX,
            end: max_scrolled - scrolled > OVERFLOW_TOLERANCE_PX,
        }
    }
}

struct EdgeFadeShader;

impl FadeShader for EdgeFadeShader {
    const LABEL: &'static str = "edge alpha fade";
    const SOURCE: &'static str = include_str!("edge_alpha_fade.wgsl");
    const FRAGMENT_ENTRY: &'static str = "fade_strength";
}

#[cfg(test)]
mod tests {
    use iced::{Color, Rectangle, Size};

    use super::{HiddenScrollEdges, ScrollEdgeFade};
    use crate::{
        alpha_fade_shaders::premultiplied_blend_constant,
        orientation::{Axis, Edge},
        theming::FrameStyle,
    };

    #[test]
    fn restored_fill_is_premultiplied_by_its_alpha() {
        let constant = premultiplied_blend_constant(Color::from_rgba(1.0, 0.5, 0.0, 0.5));
        assert!((constant.r - 0.5).abs() < 1e-6);
        assert!((constant.g - 0.25).abs() < 1e-6);
        assert!(constant.b.abs() < 1e-6);
        assert!((constant.a - 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_transparent_surface_restores_nothing() {
        let constant = premultiplied_blend_constant(Color::TRANSPARENT);
        assert!(constant.r.abs() + constant.g.abs() + constant.b.abs() + constant.a.abs() < 1e-6);
    }

    #[test]
    fn only_edges_with_hidden_content_are_faded() {
        let hidden = |scrolled_px, content_span, visible_span| {
            let edges = HiddenScrollEdges::of(scrolled_px, content_span, visible_span);
            [edges.start, edges.end]
        };
        assert_eq!(hidden(0.0, 80.0, 100.0), [false, false]);
        assert_eq!(hidden(0.0, 300.0, 100.0), [false, true]);
        assert_eq!(hidden(50.0, 300.0, 100.0), [true, true]);
        assert_eq!(hidden(500.0, 300.0, 100.0), [true, false]);
    }

    #[test]
    fn subpixel_overflow_draws_no_fade() {
        let edges = HiddenScrollEdges::of(0.0, 100.25, 100.0);
        assert_eq!([edges.start, edges.end], [false, false]);
    }

    #[test]
    fn a_band_inside_a_frame_keeps_clear_of_the_outline() {
        let frame = FrameStyle::default();
        let outline = frame.outline_width();
        assert!(outline > 0.0);
        let bounds = Rectangle::with_size(Size::new(200.0, 300.0));
        let band = ScrollEdgeFade::within_frame(frame, 28.0).band_along(Edge::Bottom, bounds);
        assert!((band.y + band.height - (300.0 - outline)).abs() < f32::EPSILON);
        assert!((band.x - outline).abs() < f32::EPSILON);
        assert!((band.height - 28.0).abs() < f32::EPSILON);
    }

    #[test]
    fn rows_stay_clear_of_the_outline_only_at_faded_edges() {
        let frame = FrameStyle::default();
        let outline = frame.outline_width();
        let clip = Rectangle::with_size(Size::new(200.0, 300.0));
        let hidden_below = HiddenScrollEdges {
            start: false,
            end: true,
        };
        let rows_clip = ScrollEdgeFade::within_frame(frame, 28.0).clear_of_outline(
            clip,
            Axis::Vertical,
            hidden_below,
        );
        assert!(rows_clip.y.abs() < f32::EPSILON);
        assert!((rows_clip.height - (300.0 - outline)).abs() < f32::EPSILON);
        assert!((rows_clip.width - 200.0).abs() < f32::EPSILON);
    }

    #[test]
    fn band_shape_packs_the_edge_and_the_radius_fraction() {
        let unpacked = |packed: u32| (packed & 3, f64::from(packed >> 2) / 65535.0);
        assert_eq!(Edge::Top.packed_with_radius_fraction(0.0), 2);
        let (edge_index, fraction) = unpacked(Edge::Bottom.packed_with_radius_fraction(0.5));
        assert_eq!(edge_index, 3);
        assert!((fraction - 0.5).abs() < 1e-4);
        let (_, clamped) = unpacked(Edge::Left.packed_with_radius_fraction(4.0));
        assert!((clamped - 1.0).abs() < 1e-9);
    }

    #[test]
    fn opposite_bands_never_overlap() {
        let frame = FrameStyle::default();
        let bounds = Rectangle::with_size(Size::new(40.0, 24.0));
        let fade = ScrollEdgeFade::away_from_frame(frame, 28.0);
        let left = fade.band_along(Edge::Left, bounds);
        let right = fade.band_along(Edge::Right, bounds);
        assert!(left.x + left.width <= right.x);
    }
}
