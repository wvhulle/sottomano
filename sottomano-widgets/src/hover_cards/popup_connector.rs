use core::f32::consts::{FRAC_PI_2, PI};

use iced::{
    Border, Color, Point, Rectangle, Shadow, Size, Theme, Transformation, Vector,
    advanced::graphics::{
        color,
        geometry::Renderer as _,
        mesh::{Indexed, Mesh, Renderer as _, SolidVertex2D},
    },
    border::Radius,
    widget::canvas::{Frame, Path, Stroke},
};
use sottomano_config::screen_edges::Position;

use crate::desktop_panels::panel_frame::LayerFrameStyle;

const BASE_HALF_WIDTH_PX: f32 = 4.0;
const SHOULDER_PX: f32 = BASE_HALF_WIDTH_PX;
const HALF_SPAN_PX: f32 = BASE_HALF_WIDTH_PX + SHOULDER_PX;
const ARC_SAMPLE_STEP_PX: f32 = 0.5;
const STRAIGHT_SAMPLE_STEP_PX: f32 = 4.0;
const CURVE_SAMPLE_COUNT: usize = 16;
const SHADOW_RING_COUNT: usize = 8;
const COINCIDENT_POINT_PX: f32 = 0.001;
const CORNER_ARC_SHARE_FOR_SHOULDER: f32 = 0.3;

#[derive(Debug, Clone, Copy)]
pub struct FramedSurface {
    pub frame: LayerFrameStyle,
    pub bounds: Rectangle,
    pub corners: Radius,
    pub shadow: Shadow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorSide {
    Top,
    Bottom,
    Left,
    Right,
}

impl From<Position> for ConnectorSide {
    fn from(position: Position) -> Self {
        match position {
            Position::Top => ConnectorSide::Top,
            Position::Bottom => ConnectorSide::Bottom,
            Position::Left => ConnectorSide::Left,
        }
    }
}

pub struct ConnectorTriangle {
    side: ConnectorSide,
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

fn toward(from: Point, to: Point, fraction: f32) -> Point {
    from + (to - from) * fraction
}

fn dot_product(first: Vector, second: Vector) -> f32 {
    first.x.mul_add(second.x, first.y * second.y)
}

fn unit_direction(from: Point, to: Point) -> Vector {
    let along = to - from;
    along * dot_product(along, along).sqrt().max(f32::EPSILON).recip()
}

#[allow(clippy::cast_precision_loss)]
fn curve_fractions() -> impl Iterator<Item = f32> {
    (1..=CURVE_SAMPLE_COUNT).map(|sample| sample as f32 / CURVE_SAMPLE_COUNT as f32)
}

fn quadratic_samples(start: Point, control: Point, end: Point) -> impl Iterator<Item = Point> {
    curve_fractions().map(move |fraction| {
        toward(
            toward(start, control, fraction),
            toward(control, end, fraction),
            fraction,
        )
    })
}

fn cubic_samples(
    start: Point,
    first_control: Point,
    second_control: Point,
    end: Point,
) -> impl Iterator<Item = Point> {
    curve_fractions().map(move |fraction| {
        toward(
            toward(
                toward(start, first_control, fraction),
                toward(first_control, second_control, fraction),
                fraction,
            ),
            toward(
                toward(first_control, second_control, fraction),
                toward(second_control, end, fraction),
                fraction,
            ),
            fraction,
        )
    })
}

struct CornerTurn {
    center: Point,
    radius: f32,
    start_angle: f32,
}

impl CornerTurn {
    fn point_at(&self, angle: f32) -> Point {
        self.center + Vector::new(angle.cos(), angle.sin()) * self.radius
    }

    fn first_point(&self) -> Point {
        self.point_at(self.start_angle)
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )]
    fn samples(&self) -> impl Iterator<Item = Point> {
        let step_count = ((self.radius * FRAC_PI_2 / ARC_SAMPLE_STEP_PX).ceil() as usize).max(1);
        (0..=step_count).map(move |step| {
            self.point_at(self.start_angle + FRAC_PI_2 * step as f32 / step_count as f32)
        })
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn straight_samples(start: Point, end: Point) -> impl Iterator<Item = Point> {
    let step_count = (start.distance(end) / STRAIGHT_SAMPLE_STEP_PX).ceil() as usize;
    (1..step_count).map(move |step| toward(start, end, step as f32 / step_count as f32))
}

struct PerimeterPosition {
    position: Point,
    tangent: Vector,
}

struct RoundedPerimeter {
    points: Vec<Point>,
    distances: Vec<f32>,
    total_length: f32,
}

impl RoundedPerimeter {
    fn around(panel: Rectangle, corners: Radius) -> Option<Self> {
        let fitted = |radius: f32| radius.clamp(0.0, panel.width.min(panel.height) / 2.0);
        let top_right = fitted(corners.top_right);
        let bottom_right = fitted(corners.bottom_right);
        let bottom_left = fitted(corners.bottom_left);
        let top_left = fitted(corners.top_left);
        let right = panel.x + panel.width;
        let bottom = panel.y + panel.height;
        let turns = [
            CornerTurn {
                center: Point::new(right - top_right, panel.y + top_right),
                radius: top_right,
                start_angle: -FRAC_PI_2,
            },
            CornerTurn {
                center: Point::new(right - bottom_right, bottom - bottom_right),
                radius: bottom_right,
                start_angle: 0.0,
            },
            CornerTurn {
                center: Point::new(panel.x + bottom_left, bottom - bottom_left),
                radius: bottom_left,
                start_angle: FRAC_PI_2,
            },
            CornerTurn {
                center: Point::new(panel.x + top_left, panel.y + top_left),
                radius: top_left,
                start_angle: PI,
            },
        ];
        let next_turns = turns.iter().cycle().skip(1);
        let sampled = turns.iter().zip(next_turns).flat_map(|(turn, next_turn)| {
            let leaving = turn.point_at(turn.start_angle + FRAC_PI_2);
            turn.samples()
                .chain(straight_samples(leaving, next_turn.first_point()))
        });
        let mut points: Vec<Point> = Vec::new();
        for point in sampled {
            if points
                .last()
                .is_none_or(|last| last.distance(point) > COINCIDENT_POINT_PX)
            {
                points.push(point);
            }
        }
        if points.len() < 2 {
            return None;
        }
        let distances = points
            .iter()
            .scan((0.0, points[0]), |(walked, previous), point| {
                *walked += previous.distance(*point);
                *previous = *point;
                Some(*walked)
            })
            .collect::<Vec<f32>>();
        let total_length =
            distances[distances.len() - 1] + points[points.len() - 1].distance(points[0]);
        Some(Self {
            points,
            distances,
            total_length,
        })
    }

    fn point_after(&self, index: usize) -> Point {
        self.points[(index + 1) % self.points.len()]
    }

    fn at(&self, distance: f32) -> PerimeterPosition {
        let wrapped = distance.rem_euclid(self.total_length);
        let index = self
            .distances
            .partition_point(|walked| *walked <= wrapped)
            .saturating_sub(1);
        let start = self.points[index];
        let end = self.point_after(index);
        let tangent = unit_direction(start, end);
        PerimeterPosition {
            position: start + tangent * (wrapped - self.distances[index]),
            tangent,
        }
    }

    fn nearest_distance(&self, target: Point) -> f32 {
        let (_, nearest) = self.points.iter().enumerate().fold(
            (f32::INFINITY, 0.0),
            |(best_gap, best_distance), (index, start)| {
                let end = self.point_after(index);
                let tangent = unit_direction(*start, end);
                let along = dot_product(target - *start, tangent).clamp(0.0, start.distance(end));
                let gap = target.distance(*start + tangent * along);
                if gap < best_gap {
                    (gap, self.distances[index] + along)
                } else {
                    (best_gap, best_distance)
                }
            },
        );
        nearest
    }

    fn points_between(&self, from: f32, to: f32) -> impl Iterator<Item = Point> {
        let sweep = (to - from).rem_euclid(self.total_length);
        let first = self
            .distances
            .partition_point(|walked| *walked <= from.rem_euclid(self.total_length));
        (0..self.points.len())
            .map(move |offset| (first + offset) % self.points.len())
            .map_while(move |index| {
                let ahead = (self.distances[index] - from).rem_euclid(self.total_length);
                (ahead < sweep).then(|| self.points[index])
            })
    }
}

fn outline_bounds(outline: &[Point]) -> Rectangle {
    let left = outline
        .iter()
        .map(|point| point.x)
        .fold(f32::INFINITY, f32::min);
    let top = outline
        .iter()
        .map(|point| point.y)
        .fold(f32::INFINITY, f32::min);
    let right = outline
        .iter()
        .map(|point| point.x)
        .fold(f32::NEG_INFINITY, f32::max);
    let bottom = outline
        .iter()
        .map(|point| point.y)
        .fold(f32::NEG_INFINITY, f32::max);
    Rectangle::new(Point::new(left, top), Size::new(right - left, bottom - top))
}

fn closed_segments(outline: &[Point]) -> impl Iterator<Item = [Point; 2]> {
    outline
        .iter()
        .zip(outline.iter().cycle().skip(1))
        .map(|(start, end)| [*start, *end])
}

#[derive(Clone, Copy)]
struct RowSpan {
    left: f32,
    right: f32,
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_cmp
)]
fn surface_scanlines(outline: &[Point]) -> Vec<Rectangle> {
    let bounds = outline_bounds(outline);
    let first_row = bounds.y.floor() as i32;
    let row_count = ((bounds.y + bounds.height).ceil() as i32 - first_row).max(0) as usize;
    let mut spans = vec![
        RowSpan {
            left: f32::INFINITY,
            right: f32::NEG_INFINITY,
        };
        row_count
    ];
    for [start, end] in closed_segments(outline) {
        let upper = start.y.min(end.y);
        let lower = start.y.max(end.y);
        let first_crossed = (upper - 0.5).ceil() as i32;
        let last_crossed = ((lower - 0.5).ceil() as i32).min(first_row + row_count as i32);
        for row in first_crossed.max(first_row)..last_crossed {
            let fraction = (row as f32 + 0.5 - start.y) / (end.y - start.y);
            let crossing = toward(start, end, fraction).x;
            let span = &mut spans[(row - first_row) as usize];
            span.left = span.left.min(crossing);
            span.right = span.right.max(crossing);
        }
    }
    let mut merged_rows: Vec<Rectangle> = Vec::new();
    for (offset, span) in spans.iter().enumerate() {
        let left = span.left.round();
        let width = span.right.round() - left;
        if width <= 0.0 {
            continue;
        }
        let top = first_row as f32 + offset as f32;
        match merged_rows.last_mut() {
            Some(previous)
                if previous.x == left
                    && previous.width == width
                    && previous.y + previous.height == top =>
            {
                previous.height += 1.0;
            }
            _ => merged_rows.push(Rectangle::new(Point::new(left, top), Size::new(width, 1.0))),
        }
    }
    merged_rows
}

fn outward_normals(outline: &[Point]) -> Vec<Vector> {
    let count = outline.len();
    (0..count)
        .map(|index| {
            let point = outline[index];
            let previous = outline[(index + count - 1) % count];
            let next = outline[(index + 1) % count];
            let heading = unit_direction(previous, point) + unit_direction(point, next);
            let length = dot_product(heading, heading).sqrt().max(f32::EPSILON);
            Vector::new(heading.y / length, -heading.x / length)
        })
        .collect()
}

fn clearance_along_normal(outline: &[Point], origin: Point, normal: Vector, reach: f32) -> f32 {
    outline.iter().fold(reach, |clearance, other| {
        let to_other = *other - origin;
        let along = dot_product(to_other, normal);
        if along > COINCIDENT_POINT_PX {
            clearance.min(dot_product(to_other, to_other) / (2.0 * along))
        } else {
            clearance
        }
    })
}

fn shadow_falloff(distance: f32, blur_radius: f32) -> f32 {
    let blend = ((distance.max(0.0) + blur_radius) / (2.0 * blur_radius)).clamp(0.0, 1.0);
    1.0 - blend * blend * (3.0 - 2.0 * blend)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn shadow_skirt_mesh(outline: &[Point], shadow: Shadow) -> Indexed<SolidVertex2D> {
    let ring_stride = SHADOW_RING_COUNT + 1;
    let vertices = outline
        .iter()
        .zip(outward_normals(outline))
        .flat_map(|(point, normal)| {
            let drop_along_normal = dot_product(shadow.offset, normal);
            let reach = (shadow.blur_radius + drop_along_normal).max(0.0);
            let clearance = clearance_along_normal(outline, *point, normal, reach);
            (0..ring_stride).map(move |ring| {
                let distance = (reach * ring as f32 / SHADOW_RING_COUNT as f32).min(clearance);
                let falloff = shadow_falloff(distance - drop_along_normal, shadow.blur_radius);
                SolidVertex2D {
                    position: (*point + normal * distance).into(),
                    color: color::pack(shadow.color.scale_alpha(falloff)),
                }
            })
        })
        .collect();
    let point_count = outline.len();
    let indices = (0..point_count)
        .flat_map(|point_index| {
            let next_index = (point_index + 1) % point_count;
            (0..SHADOW_RING_COUNT).flat_map(move |ring| {
                let inner = (point_index * ring_stride + ring) as u32;
                let outer = inner + 1;
                let next_inner = (next_index * ring_stride + ring) as u32;
                let next_outer = next_inner + 1;
                [inner, outer, next_inner, outer, next_outer, next_inner]
            })
        })
        .collect();
    Indexed { vertices, indices }
}

impl ConnectorTriangle {
    #[must_use]
    pub fn between(
        anchor: Rectangle,
        panel_edge: f32,
        popup: Rectangle,
        side: ConnectorSide,
        corner_radius: f32,
    ) -> Self {
        let inset = HALF_SPAN_PX + corner_radius;
        match side {
            ConnectorSide::Top | ConnectorSide::Bottom => {
                let base_y = if side == ConnectorSide::Top {
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
            ConnectorSide::Left | ConnectorSide::Right => {
                let base_x = if side == ConnectorSide::Left {
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
            ConnectorSide::Top | ConnectorSide::Bottom => {
                (self.base_center.x - panel.x).min(panel.x + panel.width - self.base_center.x)
            }
            ConnectorSide::Left | ConnectorSide::Right => {
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
        surface: FramedSurface,
    ) {
        let FramedSurface {
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

    use super::{ConnectorSide, ConnectorTriangle};

    const CORNER_RADIUS: f32 = 12.0;

    fn submenu_panel(height: f32) -> Rectangle {
        Rectangle::new(Point::new(100.0, 50.0), Size::new(200.0, height))
    }

    fn connector_at_row_center(panel: Rectangle) -> ConnectorTriangle {
        ConnectorTriangle::between(
            Rectangle::new(Point::new(panel.x - 14.0, panel.y + 14.0), Size::ZERO),
            panel.x - 14.0,
            panel,
            ConnectorSide::Left,
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
            ConnectorSide::Left,
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
