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
        card_origins(
            anchors,
            &sizes,
            Rectangle::with_size(SURFACE),
            &DEFAULT_ANCHOR_SPACING,
        )
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
        assert!(!Rectangle::new(origins[0], CARD).intersects(&Rectangle::new(origins[1], CARD)));
        assert_eq!(origins[0], Point::new(24.0, WINDOW_TOP));
        assert_eq!(
            origins[1],
            Point::new(
                24.0,
                WINDOW_TOP + CARD.height + DEFAULT_ANCHOR_SPACING.card_gap
            )
        );
    }

    fn window_with_bounds(id: u64, left: f32, top: f32, width: f32, height: f32) -> ToastAnchor {
        ToastAnchor::Window {
            id,
            bounds: Rectangle::new(Point::new(left, top), Size::new(width, height)),
        }
    }

    fn bottom_right_corner() -> ToastAnchor {
        ToastAnchor::Edge {
            vertical: VEdge::Bottom,
            horizontal: HEdge::Right,
        }
    }

    fn top_right_corner() -> ToastAnchor {
        ToastAnchor::Edge {
            vertical: VEdge::Top,
            horizontal: HEdge::Right,
        }
    }

    fn overlapping_cards(card_positions: &[Point]) -> Vec<Point> {
        card_positions
            .iter()
            .enumerate()
            .filter(|(index, card_position)| {
                card_positions[index + 1..].iter().any(|later_position| {
                    Rectangle::new(**card_position, CARD)
                        .intersects(&Rectangle::new(*later_position, CARD))
                })
            })
            .map(|(_, card_position)| *card_position)
            .collect()
    }

    #[test]
    fn a_stack_taller_than_its_window_clears_the_window_below() {
        let upper_window = window_with_bounds(1, 600.0, 40.0, 800.0, 200.0);
        let lower_window = window_with_bounds(2, 600.0, 240.0, 800.0, 800.0);
        let card_positions = placed(&[upper_window, upper_window, upper_window, lower_window]);
        assert!(overlapping_cards(&card_positions).is_empty());
    }

    #[test]
    fn centred_cards_of_narrow_neighbouring_windows_do_not_overlap() {
        let left_window = window_with_bounds(1, 600.0, 40.0, 200.0, 1000.0);
        let right_window = window_with_bounds(2, 800.0, 40.0, 200.0, 1000.0);
        assert!(overlapping_cards(&placed(&[left_window, right_window])).is_empty());
    }

    #[test]
    fn a_fallback_corner_stack_clears_a_window_card_in_that_corner() {
        let corner_window = window_with_bounds(1, 1100.0, 900.0, 800.0, 180.0);
        let card_positions = placed(&[corner_window, bottom_right_corner(), bottom_right_corner()]);
        assert!(overlapping_cards(&card_positions).is_empty());
    }

    #[test]
    fn cards_with_one_anchor_stack_in_index_order() {
        let card_positions = placed(&[top_right_corner(), top_right_corner()]);
        let stacked_y = card_positions[0].y + CARD.height + DEFAULT_ANCHOR_SPACING.card_gap;
        assert_eq!(
            card_positions[1],
            Point::new(card_positions[0].x, stacked_y)
        );
    }

    #[test]
    fn a_card_under_a_card_of_fractional_height_settles_below_it() {
        let fractional_card = Size::new(380.0, 90.4);
        let card_positions = card_origins(
            &[top_right_corner(), top_right_corner()],
            &[fractional_card, fractional_card],
            Rectangle::with_size(SURFACE),
            &DEFAULT_ANCHOR_SPACING,
        );
        let first_card_bottom = card_positions[0].y + fractional_card.height;
        assert_eq!(
            card_positions[1],
            Point::new(
                card_positions[0].x,
                first_card_bottom + DEFAULT_ANCHOR_SPACING.card_gap
            )
        );
    }

    #[test]
    fn a_card_over_a_card_of_fractional_height_settles_above_it() {
        let fractional_card = Size::new(380.0, 60.16);
        let card_positions = card_origins(
            &[bottom_right_corner(), bottom_right_corner()],
            &[fractional_card, fractional_card],
            Rectangle::with_size(SURFACE),
            &DEFAULT_ANCHOR_SPACING,
        );
        assert_eq!(
            card_positions[1],
            Point::new(
                card_positions[0].x,
                card_positions[0].y - DEFAULT_ANCHOR_SPACING.card_gap - fractional_card.height
            )
        );
    }

    #[test]
    fn a_bottom_anchored_stack_grows_upward() {
        let card_positions = placed(&[bottom_right_corner(), bottom_right_corner()]);
        let stacked_y = card_positions[0].y - CARD.height - DEFAULT_ANCHOR_SPACING.card_gap;
        assert_eq!(
            card_positions[1],
            Point::new(card_positions[0].x, stacked_y)
        );
    }

    #[test]
    fn a_stack_without_room_below_continues_above_its_anchor() {
        let low_window = window_with_bounds(1, 600.0, 800.0, 800.0, 240.0);
        let card_positions = placed(&[low_window, low_window, low_window]);
        assert!(overlapping_cards(&card_positions).is_empty());
        let above_first_card = card_positions[0].y - CARD.height - DEFAULT_ANCHOR_SPACING.card_gap;
        assert_eq!(
            card_positions[2],
            Point::new(card_positions[0].x, above_first_card)
        );
    }

    #[test]
    fn a_card_without_a_collision_stays_at_its_wanted_position() {
        let card_positions = placed(&[window(1, 0.0), window(2, 1000.0)]);
        assert_eq!(
            card_positions,
            vec![
                Point::new(406.0, WINDOW_TOP),
                Point::new(1406.0, WINDOW_TOP)
            ]
        );
    }

    #[test]
    fn a_card_that_fits_nowhere_keeps_its_wanted_position() {
        let crowded_window = window(1, 600.0);
        let card_positions = placed(&[crowded_window; 9]);
        assert_eq!(card_positions[8], card_positions[0]);
    }
}
