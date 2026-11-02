use std::collections::HashMap;

use iced::{Point, Rectangle, Size};
use sottomano_config::{
    Config,
    screen_edges::{HEdge, VEdge},
};

use super::toast_placement::{AnchorGroup, ToastAnchor};

#[derive(Debug, Clone)]
pub struct ToastAnchorSpacing {
    edge_gap: f32,
    card_gap: f32,
}

impl ToastAnchorSpacing {
    #[must_use]
    pub fn from_config(config: &Config) -> Self {
        Self {
            edge_gap: config.desktop_panels.height,
            card_gap: config.theme.spacing.group_gap,
        }
    }
}

#[must_use]
pub fn origins(
    anchors: &[ToastAnchor],
    sizes: &[Size],
    area: Rectangle,
    spacing: &ToastAnchorSpacing,
) -> Vec<Point> {
    let slots = share_displaced_top(
        anchors
            .iter()
            .zip(sizes)
            .map(|(&anchor, &size)| Slot::resolve(anchor, size, area, spacing))
            .collect(),
    );
    let mut placed: Vec<Placed> = grouped(&slots)
        .into_values()
        .flat_map(|members| {
            let heights: Vec<f32> = members.iter().map(|&index| sizes[index].height).collect();
            let leading = leading_offsets(&heights, spacing.card_gap);
            let extent = group_extent(&heights, &leading);
            members
                .iter()
                .zip(leading)
                .map(|(&index, leading)| Placed {
                    index,
                    origin: clamped(
                        slots[index].origin(sizes[index], leading, extent, area, spacing),
                        sizes[index],
                        area,
                        spacing,
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
        area: Rectangle,
        spacing: &ToastAnchorSpacing,
    ) -> Self {
        let ToastAnchor::Window { bounds, .. } = anchor else {
            return Self::Anchored(anchor);
        };
        let origin = window_origin(bounds, size, spacing);
        match overflowed_edge(origin.x, size, area, spacing) {
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
        area: Rectangle,
        spacing: &ToastAnchorSpacing,
    ) -> Point {
        match self {
            Self::Anchored(anchor) => member_origin(anchor, size, leading, extent, area, spacing),
            Self::Displaced { horizontal, top } => {
                Point::new(edge_x(horizontal, size, area, spacing), top + leading)
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

fn leading_offsets(heights: &[f32], card_gap: f32) -> Vec<f32> {
    heights
        .iter()
        .scan(0.0, |running, height| {
            let leading = *running;
            *running += height + card_gap;
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
    area: Rectangle,
    spacing: &ToastAnchorSpacing,
) -> Point {
    let gap = spacing.edge_gap;
    match anchor {
        ToastAnchor::Edge {
            vertical,
            horizontal,
        } => {
            let y = match vertical {
                VEdge::Top => area.y + gap + leading,
                VEdge::Bottom => area.y + area.height - gap - leading - size.height,
                VEdge::Center => area.y + (area.height - extent) / 2.0 + leading,
            };
            Point::new(edge_x(horizontal, size, area, spacing), y)
        }
        ToastAnchor::Window { bounds, .. } => {
            let origin = window_origin(bounds, size, spacing);
            Point::new(origin.x, origin.y + leading)
        }
    }
}

fn edge_x(horizontal: HEdge, size: Size, area: Rectangle, spacing: &ToastAnchorSpacing) -> f32 {
    let gap = spacing.edge_gap;
    match horizontal {
        HEdge::Left => area.x + gap,
        HEdge::Right => area.x + area.width - size.width - gap,
        HEdge::Center => area.x + (area.width - size.width) / 2.0,
    }
}

fn window_origin(bounds: Rectangle, size: Size, spacing: &ToastAnchorSpacing) -> Point {
    let from_right = spacing.card_gap;
    let x = if bounds.width >= size.width + 2.0 * from_right {
        bounds.x + bounds.width - size.width - from_right
    } else {
        bounds.center_x() - size.width / 2.0
    };
    Point::new(x, bounds.y + spacing.edge_gap)
}

fn overflowed_edge(
    x: f32,
    size: Size,
    area: Rectangle,
    spacing: &ToastAnchorSpacing,
) -> Option<HEdge> {
    let gap = spacing.edge_gap;
    if x < area.x + gap {
        Some(HEdge::Left)
    } else if x > area.x + area.width - size.width - gap {
        Some(HEdge::Right)
    } else {
        None
    }
}

fn clamped(origin: Point, size: Size, area: Rectangle, spacing: &ToastAnchorSpacing) -> Point {
    let gap = spacing.edge_gap;
    let left = area.x + gap;
    let top = area.y + gap;
    Point::new(
        origin
            .x
            .clamp(left, (area.x + area.width - size.width - gap).max(left)),
        origin
            .y
            .clamp(top, (area.y + area.height - size.height - gap).max(top)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SURFACE: Size = Size::new(1920.0, 1080.0);
    const CARD: Size = Size::new(380.0, 100.0);
    const WINDOW_TOP: f32 = 64.0;
    const DEFAULT_ANCHOR_SPACING: ToastAnchorSpacing = ToastAnchorSpacing {
        edge_gap: 24.0,
        card_gap: 14.0,
    };

    fn window(id: u64, x: f32) -> ToastAnchor {
        ToastAnchor::Window {
            id,
            bounds: Rectangle::new(Point::new(x, 40.0), Size::new(800.0, 1000.0)),
        }
    }

    fn placed(anchors: &[ToastAnchor]) -> Vec<Point> {
        let sizes = vec![CARD; anchors.len()];
        origins(
            anchors,
            &sizes,
            Rectangle::with_size(SURFACE),
            &DEFAULT_ANCHOR_SPACING,
        )
    }

    fn overlap(a: Point, b: Point) -> bool {
        Rectangle::new(a, CARD).intersects(&Rectangle::new(b, CARD))
    }

    #[test]
    fn scrolling_a_window_off_screen_moves_its_toast_sideways_only() {
        assert_eq!(
            placed(&[window(1, 600.0)])[0],
            Point::new(1006.0, WINDOW_TOP)
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
            Point::new(
                24.0,
                WINDOW_TOP + CARD.height + DEFAULT_ANCHOR_SPACING.card_gap
            )
        );
    }
}
