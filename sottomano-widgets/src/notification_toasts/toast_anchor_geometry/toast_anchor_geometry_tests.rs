use super::*;

const SURFACE: Size = Size::new(1920.0, 1080.0);
const CARD: Size = Size::new(380.0, 100.0);
const WINDOW_TOP: f32 = 64.0;
const DEFAULT_ANCHOR_SPACING: ToastAnchorSpacing = ToastAnchorSpacing {
    edge_gap: 24.0,
    card_gap: 14.0,
};

fn window(id: u64, left_edge: f32) -> ToastAnchor {
    ToastAnchor::Window {
        id,
        bounds: Rectangle::new(Point::new(left_edge, 40.0), Size::new(800.0, 1000.0)),
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
