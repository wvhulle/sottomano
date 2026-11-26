mod outline_rasterizing;
mod perimeter_curve_sampling;

use core::f32::consts::FRAC_PI_2;

use iced::{
    Border, Color, Point, Rectangle, Shadow, Theme, Transformation, Vector,
    advanced::graphics::{
        geometry::Renderer as _,
        mesh::{Mesh, Renderer as _},
    },
    border::Radius,
    widget::canvas::{Frame, Path, Stroke},
};

use self::{
    outline_rasterizing::{outline_bounds, shadow_skirt_mesh, surface_scanlines},
    perimeter_curve_sampling::{RoundedPerimeter, cubic_samples, quadratic_samples, toward},
};
use crate::{orientation::Edge, theming::FrameStyle};

const BASE_HALF_WIDTH_PX: f32 = 3.0;
const SHOULDER_PX: f32 = BASE_HALF_WIDTH_PX / 2.0;
const HALF_SPAN_PX: f32 = BASE_HALF_WIDTH_PX + SHOULDER_PX;
const ARC_SAMPLE_STEP_PX: f32 = 0.5;
const STRAIGHT_SAMPLE_STEP_PX: f32 = 4.0;
const CURVE_SAMPLE_COUNT: usize = 16;
const SHADOW_RING_COUNT: usize = 8;
const COINCIDENT_POINT_PX: f32 = 0.001;
const CORNER_ARC_SHARE_FOR_SHOULDER: f32 = 0.3;

#[derive(Debug, Clone, Copy)]
pub struct ConnectedSurface {
    pub frame: FrameStyle,
    pub bounds: Rectangle,
    pub corners: Radius,
    pub shadow: Shadow,
}

pub struct ConnectorTriangle {
    side: Edge,
    apex: Point,
    base_center: Point,
}

fn centered_within(target: f32, start: f32, end: f32, inset: f32) -> f32 {
    let first = start + inset;
    let last = end - inset;
    if first <= last {
        target.clamp(first, last)
    } else {
        f32::midpoint(start, end)
    }
}

impl ConnectorTriangle {
    #[must_use]
    pub fn between(
        anchor: Rectangle,
        panel_edge: f32,
        popup: Rectangle,
        side: Edge,
        corner_radius: f32,
    ) -> Self {
        let inset = HALF_SPAN_PX + corner_radius;
        match side {
            Edge::Top | Edge::Bottom => {
                let base_y = if side == Edge::Top {
                    popup.y
                } else {
                    popup.y + popup.height
                };
                let base_center =
                    centered_within(anchor.center_x(), popup.x, popup.x + popup.width, inset);
                Self {
                    side,
                    apex: Point::new(anchor.center_x(), panel_edge),
                    base_center: Point::new(base_center, base_y),
                }
            }
            Edge::Left | Edge::Right => {
                let base_x = if side == Edge::Left {
                    popup.x
                } else {
                    popup.x + popup.width
                };
                let base_center =
                    centered_within(anchor.center_y(), popup.y, popup.y + popup.height, inset);
                Self {
                    side,
                    apex: Point::new(panel_edge, anchor.center_y()),
                    base_center: Point::new(base_x, base_center),
                }
            }
        }
    }

    #[must_use]
    pub fn translated(&self, offset: Vector) -> Self {
        Self {
            side: self.side,
            apex: self.apex + offset,
            base_center: self.base_center + offset,
        }
    }

    fn rounded_tip_point(&self, base: Point) -> Point {
        let tip_share = (SHOULDER_PX / self.apex.distance(base).max(f32::EPSILON)).min(0.5);
        toward(self.apex, base, tip_share)
    }

    fn half_span_keeping_corners(&self, panel: Rectangle, corners: Radius) -> f32 {
        let room_to_side_end = match self.side {
            Edge::Top | Edge::Bottom => {
                (self.base_center.x - panel.x).min(panel.x + panel.width - self.base_center.x)
            }
            Edge::Left | Edge::Right => {
                (self.base_center.y - panel.y).min(panel.y + panel.height - self.base_center.y)
            }
        };
        let widest_radius = corners
            .top_left
            .max(corners.top_right)
            .max(corners.bottom_right)
            .max(corners.bottom_left)
            .min(panel.width.min(panel.height) / 2.0);
        let corner_arc_length = widest_radius * FRAC_PI_2;
        HALF_SPAN_PX.min(
            room_to_side_end - widest_radius + corner_arc_length * CORNER_ARC_SHARE_FOR_SHOULDER,
        )
    }

    fn joined_outline_points(&self, panel: Rectangle, corners: Radius) -> Option<Vec<Point>> {
        let perimeter = RoundedPerimeter::around(panel, corners)?;
        let center = perimeter.nearest_distance(self.base_center);
        let half_span = self
            .half_span_keeping_corners(panel, corners)
            .min(perimeter.total_length / 4.0)
            .max(COINCIDENT_POINT_PX);
        let shoulder_length = half_span / 2.0;
        let entry = perimeter.at(center - half_span);
        let exit = perimeter.at(center + half_span);
        let entry_base = perimeter.at(center - shoulder_length).position;
        let exit_base = perimeter.at(center + shoulder_length).position;
        let entry_tip = self.rounded_tip_point(entry_base);
        let exit_tip = self.rounded_tip_point(exit_base);
        let outline = perimeter
            .points_between(center + half_span, center - half_span)
            .chain([entry.position])
            .chain(cubic_samples(
                entry.position,
                entry.position + entry.tangent * shoulder_length,
                toward(entry_base, self.apex, 0.5),
                entry_tip,
            ))
            .chain(quadratic_samples(entry_tip, self.apex, exit_tip))
            .chain(cubic_samples(
                exit_tip,
                toward(exit_base, self.apex, 0.5),
                exit.position - exit.tangent * shoulder_length,
                exit.position,
            ))
            .collect();
        Some(outline)
    }

    #[must_use]
    pub fn surface_rows(&self, panel: Rectangle, corners: Radius) -> Vec<Rectangle> {
        if panel.width <= 0.0 || panel.height <= 0.0 {
            return Vec::new();
        }
        self.joined_outline_points(panel, corners)
            .map_or_else(Vec::new, |outline| surface_scanlines(&outline))
    }

    pub fn draw_framed_surface(
        &self,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        surface: ConnectedSurface,
    ) {
        let ConnectedSurface {
            frame,
            bounds,
            corners,
            shadow,
        } = surface;
        let Some(outline) = self.joined_outline_points(bounds, corners) else {
            return;
        };
        let drawn = outline_bounds(&outline);
        if shadow.color.a > 0.0 && shadow.blur_radius > 0.0 {
            let shadow_reach =
                shadow.blur_radius + shadow.offset.x.abs().max(shadow.offset.y.abs());
            renderer.draw_mesh(Mesh::Solid {
                buffers: shadow_skirt_mesh(&outline, shadow),
                transformation: Transformation::IDENTITY,
                clip_bounds: drawn.expand(shadow_reach + 1.0),
            });
        }
        Self::draw_joined(
            renderer,
            &outline,
            drawn,
            frame.fill_color(theme),
            frame.outline_border(theme),
        );
    }

    fn draw_joined(
        renderer: &mut iced::Renderer,
        outline: &[Point],
        drawn: Rectangle,
        fill: Color,
        outline_border: Border,
    ) {
        let reach = outline_border.width.max(1.0);
        let mut frame = Frame::with_bounds(renderer, drawn.expand(reach));
        let surface = Path::new(|path| {
            path.move_to(outline[0]);
            outline[1..].iter().for_each(|point| path.line_to(*point));
            path.close();
        });
        frame.fill(&surface, fill);
        if outline_border.width > 0.0 {
            frame.stroke(
                &surface,
                Stroke::default()
                    .with_color(outline_border.color)
                    .with_width(outline_border.width),
            );
        }
        renderer.draw_geometry(frame.into_geometry());
    }
}

#[cfg(test)]
mod tests {
    use iced::{Point, Rectangle, Size, border::Radius};

    use super::ConnectorTriangle;
    use crate::orientation::Edge;

    const CORNER_RADIUS: f32 = 12.0;

    fn submenu_panel(height: f32) -> Rectangle {
        Rectangle::new(Point::new(100.0, 50.0), Size::new(200.0, height))
    }

    fn connector_at_row_center(panel: Rectangle) -> ConnectorTriangle {
        ConnectorTriangle::between(
            Rectangle::new(Point::new(panel.x - 14.0, panel.y + 14.0), Size::ZERO),
            panel.x - 14.0,
            panel,
            Edge::Left,
            0.0,
        )
    }

    #[test]
    fn outline_of_a_single_row_panel_keeps_clear_of_the_corner_vertices() {
        let panel = submenu_panel(28.0);
        let outline = connector_at_row_center(panel)
            .joined_outline_points(panel, Radius::from(CORNER_RADIUS))
            .unwrap();
        let corner_clearance = CORNER_RADIUS * (2.0_f32.sqrt() - 1.0) * 0.5;
        for corner in [
            Point::new(panel.x, panel.y),
            Point::new(panel.x, panel.y + panel.height),
        ] {
            let nearest = outline
                .iter()
                .map(|point| point.distance(corner))
                .fold(f32::INFINITY, f32::min);
            assert!(
                nearest > corner_clearance,
                "outline passes {nearest}px from {corner:?}",
            );
        }
    }

    #[allow(clippy::float_cmp)]
    #[test]
    fn surface_rows_reach_from_the_panel_edge_to_the_connector_without_gaps() {
        let panel = submenu_panel(28.0);
        let rows = connector_at_row_center(panel).surface_rows(panel, Radius::from(CORNER_RADIUS));
        let covered_height: f32 = rows.iter().map(|row| row.height).sum();
        assert_eq!(covered_height, panel.height);
        for row_pair in rows.windows(2) {
            assert_eq!(row_pair[0].y + row_pair[0].height, row_pair[1].y);
        }
        let center_row = rows
            .iter()
            .find(|row| row.y <= panel.center_y() && panel.center_y() < row.y + row.height)
            .unwrap();
        assert!(center_row.x < panel.x - 10.0);
        assert_eq!(center_row.x + center_row.width, panel.x + panel.width);
    }

    #[test]
    fn shoulders_on_a_tall_panel_stay_on_the_straight_side() {
        let panel = submenu_panel(300.0);
        let connector = ConnectorTriangle::between(
            Rectangle::new(Point::new(panel.x - 14.0, panel.center_y()), Size::ZERO),
            panel.x - 14.0,
            panel,
            Edge::Left,
            CORNER_RADIUS,
        );
        let outline = connector
            .joined_outline_points(panel, Radius::from(CORNER_RADIUS))
            .unwrap();
        let side_points = outline
            .iter()
            .filter(|point| {
                (point.x - panel.x).abs() < 0.001
                    && (point.y - panel.center_y()).abs() > super::HALF_SPAN_PX
                    && (point.y - panel.center_y()).abs() < panel.height / 2.0 - CORNER_RADIUS
            })
            .count();
        assert!(side_points > 0);
        assert!(outline.iter().all(|point| point.x >= panel.x - 14.0));
    }

    #[test]
    fn closing_panel_outline_stays_finite_at_every_size() {
        for remaining_permille in 1_u8..=50 {
            let shrink = (f32::from(remaining_permille) / 1000.0).powi(3);
            let panel = Rectangle::new(
                Point::new(100.0, 50.0),
                Size::new(400.0 * shrink, 300.0 * shrink),
            );
            let outline = connector_at_row_center(panel)
                .joined_outline_points(panel, Radius::from(CORNER_RADIUS))
                .unwrap_or_default();
            assert!(
                outline
                    .iter()
                    .all(|point| point.x.is_finite() && point.y.is_finite()),
                "non-finite outline for {panel:?}",
            );
        }
    }

    #[test]
    fn panel_smaller_than_point_spacing_has_no_outline() {
        let panel = Rectangle::new(Point::new(100.0, 50.0), Size::new(0.0004, 0.0003));
        let connector = connector_at_row_center(panel);
        let corners = Radius::from(CORNER_RADIUS);
        assert!(connector.joined_outline_points(panel, corners).is_none());
        assert!(connector.surface_rows(panel, corners).is_empty());
    }

    #[test]
    fn collapsed_panel_has_no_surface_rows() {
        let panel = submenu_panel(0.0);
        let rows = connector_at_row_center(panel).surface_rows(panel, Radius::from(CORNER_RADIUS));
        assert!(rows.is_empty());
    }
}
