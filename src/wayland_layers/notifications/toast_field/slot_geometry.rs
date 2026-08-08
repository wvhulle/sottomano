use std::collections::HashMap;

use iced::{Point, Size};

use crate::{
    config::NotificationConfig,
    data::source::niri::{HEdge, VEdge},
    wayland_layers::notifications::toast_anchor::{AnchorGroup, ToastAnchor},
};

pub const STACK_SPACING: f32 = 8.0;

pub fn origins(
    anchors: &[ToastAnchor],
    sizes: &[Size],
    surface: Size,
    config: &NotificationConfig,
) -> Vec<Point> {
    let mut placed: Vec<Placed> = grouped(anchors)
        .into_values()
        .flat_map(|members| {
            let heights: Vec<f32> = members.iter().map(|&index| sizes[index].height).collect();
            let leading = leading_offsets(&heights);
            let extent = group_extent(&heights, &leading);
            members
                .iter()
                .zip(leading)
                .map(|(&index, leading)| Placed {
                    index,
                    origin: clamped(
                        member_origin(
                            anchors[index],
                            sizes[index],
                            leading,
                            extent,
                            surface,
                            config,
                        ),
                        sizes[index],
                        surface,
                        config,
                    ),
                })
                .collect::<Vec<_>>()
        })
        .collect();
    placed.sort_unstable_by_key(|slot| slot.index);
    placed.into_iter().map(|slot| slot.origin).collect()
}

struct Placed {
    index: usize,
    origin: Point,
}

fn grouped(anchors: &[ToastAnchor]) -> HashMap<AnchorGroup, Vec<usize>> {
    anchors
        .iter()
        .enumerate()
        .fold(HashMap::new(), |mut groups, (index, anchor)| {
            groups.entry(anchor.group()).or_default().push(index);
            groups
        })
}

fn leading_offsets(heights: &[f32]) -> Vec<f32> {
    heights
        .iter()
        .scan(0.0, |running, height| {
            let leading = *running;
            *running += height + STACK_SPACING;
            Some(leading)
        })
        .collect()
}

fn group_extent(heights: &[f32], leading: &[f32]) -> f32 {
    heights
        .last()
        .zip(leading.last())
        .map_or(0.0, |(height, leading)| leading + height)
}

fn member_origin(
    anchor: ToastAnchor,
    size: Size,
    leading: f32,
    extent: f32,
    surface: Size,
    config: &NotificationConfig,
) -> Point {
    let gap = config.edge_gap;
    match anchor {
        ToastAnchor::Edge {
            vertical,
            horizontal,
        } => {
            let x = match horizontal {
                HEdge::Left => gap,
                HEdge::Right => surface.width - size.width - gap,
                HEdge::Center => (surface.width - size.width) / 2.0,
            };
            let y = match vertical {
                VEdge::Top => gap + leading,
                VEdge::Bottom => surface.height - gap - leading - size.height,
                VEdge::Center => (surface.height - extent) / 2.0 + leading,
            };
            Point::new(x, y)
        }
        ToastAnchor::Window { bounds, .. } => {
            let from_right = config.window_right_offset;
            let x = if bounds.width >= size.width + 2.0 * from_right {
                bounds.x + bounds.width - size.width - from_right
            } else {
                bounds.center_x() - size.width / 2.0
            };
            Point::new(x, bounds.y + config.window_top_offset + leading)
        }
    }
}

fn clamped(origin: Point, size: Size, surface: Size, config: &NotificationConfig) -> Point {
    let gap = config.edge_gap;
    Point::new(
        origin
            .x
            .clamp(gap, (surface.width - size.width - gap).max(gap)),
        origin
            .y
            .clamp(gap, (surface.height - size.height - gap).max(gap)),
    )
}
