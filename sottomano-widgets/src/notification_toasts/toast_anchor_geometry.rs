use std::iter::once;

use iced::{Point, Rectangle, Size};
use sottomano_toml_config::{
    Config,
    screen_edges::{HEdge, VEdge},
};

use super::toast_placement::ToastAnchor;

#[derive(Debug, Clone)]
pub struct ToastAnchorSpacing {
    edge_gap: f32,
    card_gap: f32,
}

impl From<&Config> for ToastAnchorSpacing {
    fn from(config: &Config) -> Self {
        Self {
            edge_gap: config.desktop_panels.height,
            card_gap: config.theme.spacing.group_gap,
        }
    }
}

#[must_use]
pub fn card_origins(
    anchors: &[ToastAnchor],
    card_sizes: &[Size],
    free_area: Rectangle,
    spacing: &ToastAnchorSpacing,
) -> Vec<Point> {
    let settled_cards = anchors.iter().zip(card_sizes).fold(
        Vec::with_capacity(anchors.len()),
        |mut settled_cards: Vec<Rectangle>, (&anchor, &card_size)| {
            let wanted_card = Rectangle::new(
                wanted_origin(anchor, card_size, free_area, spacing),
                card_size,
            );
            let stacking = StackingDirection::from(anchor);
            let free_card = [stacking, stacking.opposite()]
                .into_iter()
                .filter_map(|direction| {
                    nearest_free_card(wanted_card, direction, &settled_cards, spacing.card_gap)
                })
                .find(|free_card| {
                    free_card.y >= free_area.y + spacing.edge_gap
                        && free_card.y + free_card.height
                            <= free_area.y + free_area.height - spacing.edge_gap
                })
                .unwrap_or(wanted_card);
            settled_cards.push(free_card);
            settled_cards
        },
    );
    settled_cards.iter().map(Rectangle::position).collect()
}

#[derive(Clone, Copy)]
enum StackingDirection {
    Downward,
    Upward,
}

impl StackingDirection {
    fn opposite(self) -> Self {
        match self {
            Self::Downward => Self::Upward,
            Self::Upward => Self::Downward,
        }
    }

    fn clearing_top(self, blocked_range: BlockedTopRange) -> f32 {
        match self {
            Self::Downward => blocked_range.first_top_below,
            Self::Upward => blocked_range.last_top_above,
        }
    }

    fn nearest_top(self, candidate_tops: impl Iterator<Item = f32>) -> Option<f32> {
        match self {
            Self::Downward => candidate_tops.min_by(f32::total_cmp),
            Self::Upward => candidate_tops.max_by(f32::total_cmp),
        }
    }
}

impl From<ToastAnchor> for StackingDirection {
    fn from(anchor: ToastAnchor) -> Self {
        match anchor {
            ToastAnchor::Edge {
                vertical: VEdge::Bottom,
                ..
            } => Self::Upward,
            ToastAnchor::Edge { .. } | ToastAnchor::Window { .. } => Self::Downward,
        }
    }
}

#[derive(Clone, Copy)]
struct BlockedTopRange {
    last_top_above: f32,
    first_top_below: f32,
}

impl BlockedTopRange {
    fn around_settled_card(settled_card: Rectangle, card_height: f32, card_gap: f32) -> Self {
        Self {
            last_top_above: settled_card.y - card_gap - card_height,
            first_top_below: settled_card.y + settled_card.height + card_gap,
        }
    }
}

fn nearest_free_card(
    wanted_card: Rectangle,
    direction: StackingDirection,
    settled_cards: &[Rectangle],
    card_gap: f32,
) -> Option<Rectangle> {
    let blocked_ranges: Vec<_> = settled_cards
        .iter()
        .filter(|settled_card| {
            let overlap_left = (wanted_card.x - card_gap).max(settled_card.x);
            let overlap_right = (wanted_card.x + wanted_card.width + card_gap)
                .min(settled_card.x + settled_card.width);
            overlap_right > overlap_left
        })
        .map(|settled_card| {
            BlockedTopRange::around_settled_card(*settled_card, wanted_card.height, card_gap)
        })
        .collect();
    let candidate_tops = once(wanted_card.y).chain(
        blocked_ranges
            .iter()
            .map(|blocked_range| direction.clearing_top(*blocked_range)),
    );
    let free_tops = candidate_tops
        .filter(|candidate_top| match direction {
            StackingDirection::Downward => *candidate_top >= wanted_card.y,
            StackingDirection::Upward => *candidate_top <= wanted_card.y,
        })
        .filter(|candidate_top| {
            blocked_ranges.iter().all(|blocked_range| {
                *candidate_top <= blocked_range.last_top_above
                    || *candidate_top >= blocked_range.first_top_below
            })
        });
    direction.nearest_top(free_tops).map(|free_top| Rectangle {
        y: free_top,
        ..wanted_card
    })
}

fn wanted_origin(
    anchor: ToastAnchor,
    card_size: Size,
    free_area: Rectangle,
    spacing: &ToastAnchorSpacing,
) -> Point {
    let unclamped_origin = match anchor {
        ToastAnchor::Edge {
            vertical,
            horizontal,
        } => Point::new(
            edge_x(horizontal, card_size, free_area, spacing),
            edge_y(vertical, card_size, free_area, spacing),
        ),
        ToastAnchor::Window { bounds, .. } => {
            let window_card_origin = window_origin(bounds, card_size, spacing);
            overflowed_edge(window_card_origin.x, card_size, free_area, spacing).map_or(
                window_card_origin,
                |horizontal| {
                    Point::new(
                        edge_x(horizontal, card_size, free_area, spacing),
                        window_card_origin.y,
                    )
                },
            )
        }
    };
    clamped(unclamped_origin, card_size, free_area, spacing)
}

fn edge_y(
    vertical: VEdge,
    card_size: Size,
    free_area: Rectangle,
    spacing: &ToastAnchorSpacing,
) -> f32 {
    match vertical {
        VEdge::Top => free_area.y + spacing.edge_gap,
        VEdge::Bottom => free_area.y + free_area.height - spacing.edge_gap - card_size.height,
        VEdge::Center => free_area.y + (free_area.height - card_size.height) / 2.0,
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
    let left_edge = if bounds.width >= size.width + 2.0 * from_right {
        bounds.x + bounds.width - size.width - from_right
    } else {
        bounds.center_x() - size.width / 2.0
    };
    Point::new(left_edge, bounds.y + spacing.edge_gap)
}

fn overflowed_edge(
    left_edge: f32,
    size: Size,
    area: Rectangle,
    spacing: &ToastAnchorSpacing,
) -> Option<HEdge> {
    let gap = spacing.edge_gap;
    if left_edge < area.x + gap {
        Some(HEdge::Left)
    } else if left_edge > area.x + area.width - size.width - gap {
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
mod toast_anchor_geometry_tests;
