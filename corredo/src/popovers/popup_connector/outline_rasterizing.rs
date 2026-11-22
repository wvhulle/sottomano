use iced::{
    Point, Rectangle, Shadow, Size, Vector,
    advanced::graphics::{
        color,
        mesh::{Indexed, SolidVertex2D},
    },
};

use super::{
    COINCIDENT_POINT_PX, SHADOW_RING_COUNT,
    perimeter_curve_sampling::{dot_product, toward, unit_direction},
};

pub(super) fn outline_bounds(outline: &[Point]) -> Rectangle {
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
pub(super) fn surface_scanlines(outline: &[Point]) -> Vec<Rectangle> {
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
pub(super) fn shadow_skirt_mesh(outline: &[Point], shadow: Shadow) -> Indexed<SolidVertex2D> {
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
