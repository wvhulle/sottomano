use std::collections::HashMap;

use iced::{Point, Rectangle, Size};

use crate::{
    config::toasts::ToastPlacementConfig,
    data::source::niri::{HEdge, VEdge},
    wayland_layers::notifications::toast_anchor::{AnchorGroup, ToastAnchor},
};

pub const STACK_SPACING: f32 = 8.0;

pub fn origins(
    anchors: &[ToastAnchor],
    sizes: &[Size],
    surface: Size,
    config: &ToastPlacementConfig,
) -> Vec<Point> {
    let slots = share_displaced_top(
        anchors
            .iter()
            .zip(sizes)
            .map(|(&anchor, &size)| Slot::resolve(anchor, size, surface, config))
            .collect(),
    );
    let mut placed: Vec<Placed> = grouped(&slots)
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
                        slots[index].origin(sizes[index], leading, extent, surface, config),
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

#[derive(Clone, Copy)]
enum Slot {
    Anchored(ToastAnchor),
    Displaced { horizontal: HEdge, top: f32 },
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum SlotGroup {
    Anchored(AnchorGroup),
    Displaced(HEdge),
}

impl Slot {
    fn resolve(
        anchor: ToastAnchor,
        size: Size,
        surface: Size,
        config: &ToastPlacementConfig,
    ) -> Self {
        let ToastAnchor::Window { bounds, .. } = anchor else {
            return Self::Anchored(anchor);
        };
        let origin = window_origin(bounds, size, config);
        match overflowed_edge(origin.x, size, surface, config) {
            Some(horizontal) => Self::Displaced {
                horizontal,
                top: origin.y,
            },
            None => Self::Anchored(anchor),
        }
    }

    fn group(self) -> SlotGroup {
        match self {
            Self::Anchored(anchor) => SlotGroup::Anchored(anchor.group()),
            Self::Displaced { horizontal, .. } => SlotGroup::Displaced(horizontal),
        }
    }

    fn origin(
        self,
        size: Size,
        leading: f32,
        extent: f32,
        surface: Size,
        config: &ToastPlacementConfig,
    ) -> Point {
        match self {
            Self::Anchored(anchor) => member_origin(anchor, size, leading, extent, surface, config),
            Self::Displaced { horizontal, top } => {
                Point::new(edge_x(horizontal, size, surface, config), top + leading)
            }
        }
    }
}

fn share_displaced_top(mut slots: Vec<Slot>) -> Vec<Slot> {
    let highest: HashMap<HEdge, f32> = slots.iter().fold(HashMap::new(), |mut tops, slot| {
        if let Slot::Displaced { horizontal, top } = slot {
            tops.entry(*horizontal)
                .and_modify(|shared| *shared = shared.min(*top))
                .or_insert(*top);
        }
        tops
    });
    slots.iter_mut().for_each(|slot| {
        if let Slot::Displaced { horizontal, top } = slot {
            *top = highest[horizontal];
        }
    });
    slots
}

fn grouped(slots: &[Slot]) -> HashMap<SlotGroup, Vec<usize>> {
    slots
        .iter()
        .enumerate()
        .fold(HashMap::new(), |mut groups, (index, slot)| {
            groups.entry(slot.group()).or_default().push(index);
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
    config: &ToastPlacementConfig,
) -> Point {
    let gap = config.edge_gap_px;
    match anchor {
        ToastAnchor::Edge {
            vertical,
            horizontal,
        } => {
            let y = match vertical {
                VEdge::Top => gap + leading,
                VEdge::Bottom => surface.height - gap - leading - size.height,
                VEdge::Center => (surface.height - extent) / 2.0 + leading,
            };
            Point::new(edge_x(horizontal, size, surface, config), y)
        }
        ToastAnchor::Window { bounds, .. } => {
            let origin = window_origin(bounds, size, config);
            Point::new(origin.x, origin.y + leading)
        }
    }
}

fn edge_x(horizontal: HEdge, size: Size, surface: Size, config: &ToastPlacementConfig) -> f32 {
    let gap = config.edge_gap_px;
    match horizontal {
        HEdge::Left => gap,
        HEdge::Right => surface.width - size.width - gap,
        HEdge::Center => (surface.width - size.width) / 2.0,
    }
}

fn window_origin(bounds: Rectangle, size: Size, config: &ToastPlacementConfig) -> Point {
    let from_right = config.window_right_offset_px;
    let x = if bounds.width >= size.width + 2.0 * from_right {
        bounds.x + bounds.width - size.width - from_right
    } else {
        bounds.center_x() - size.width / 2.0
    };
    Point::new(x, bounds.y + config.window_top_offset_px)
}

fn overflowed_edge(
    x: f32,
    size: Size,
    surface: Size,
    config: &ToastPlacementConfig,
) -> Option<HEdge> {
    let gap = config.edge_gap_px;
    if x < gap {
        Some(HEdge::Left)
    } else if x > surface.width - size.width - gap {
        Some(HEdge::Right)
    } else {
        None
    }
}

fn clamped(origin: Point, size: Size, surface: Size, config: &ToastPlacementConfig) -> Point {
    let gap = config.edge_gap_px;
    Point::new(
        origin
            .x
            .clamp(gap, (surface.width - size.width - gap).max(gap)),
        origin
            .y
            .clamp(gap, (surface.height - size.height - gap).max(gap)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SURFACE: Size = Size::new(1920.0, 1080.0);
    const CARD: Size = Size::new(380.0, 100.0);
    const WINDOW_TOP: f32 = 88.0;

    fn window(id: u64, x: f32) -> ToastAnchor {
        ToastAnchor::Window {
            id,
            bounds: Rectangle::new(Point::new(x, 40.0), Size::new(800.0, 1000.0)),
        }
    }

    fn placed(anchors: &[ToastAnchor]) -> Vec<Point> {
        let sizes = vec![CARD; anchors.len()];
        origins(anchors, &sizes, SURFACE, &ToastPlacementConfig::default())
    }

    fn overlap(a: Point, b: Point) -> bool {
        Rectangle::new(a, CARD).intersects(&Rectangle::new(b, CARD))
    }

    #[test]
    fn scrolling_a_window_off_screen_moves_its_toast_sideways_only() {
        assert_eq!(
            placed(&[window(1, 600.0)])[0],
            Point::new(1008.0, WINDOW_TOP)
        );
        assert_eq!(
            placed(&[window(1, -1300.0)])[0],
            Point::new(24.0, WINDOW_TOP)
        );
    }

    #[test]
    fn toasts_of_windows_scrolled_off_screen_stack_instead_of_piling_up() {
        let origins = placed(&[window(1, -1300.0), window(2, -500.0)]);
        assert!(!overlap(origins[0], origins[1]));
        assert_eq!(origins[0], Point::new(24.0, WINDOW_TOP));
        assert_eq!(
            origins[1],
            Point::new(24.0, WINDOW_TOP + CARD.height + STACK_SPACING)
        );
    }
}
