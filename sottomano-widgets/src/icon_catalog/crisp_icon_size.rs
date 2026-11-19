const SMALLEST_FRACTION_OF_TARGET: f32 = 0.75;

#[must_use]
pub fn crisp_icon_side_px(design_side_px: f32, target_side_px: f32) -> f32 {
    if design_side_px <= target_side_px {
        return design_side_px * (target_side_px / design_side_px).floor();
    }
    let whole_fraction_px = design_side_px / (design_side_px / target_side_px).ceil();
    if whole_fraction_px >= target_side_px * SMALLEST_FRACTION_OF_TARGET {
        whole_fraction_px
    } else {
        target_side_px
    }
}

#[cfg(test)]
mod tests {
    use super::crisp_icon_side_px;

    fn drawn_side(design_side_px: f32, target_side_px: f32) -> f32 {
        crisp_icon_side_px(design_side_px, target_side_px)
    }

    #[test]
    fn small_designs_draw_at_whole_multiples_of_their_size() {
        assert!((drawn_side(16.0, 17.7) - 16.0).abs() < f32::EPSILON);
        assert!((drawn_side(16.0, 33.0) - 32.0).abs() < f32::EPSILON);
        assert!((drawn_side(22.0, 24.0) - 22.0).abs() < f32::EPSILON);
    }

    #[test]
    fn large_designs_draw_at_whole_fractions_of_their_size() {
        assert!((drawn_side(48.0, 17.7) - 16.0).abs() < f32::EPSILON);
        assert!((drawn_side(32.0, 24.0) - 24.0).abs() < f32::EPSILON);
    }

    #[test]
    fn a_design_slightly_above_the_target_shrinks_to_the_target() {
        assert!((drawn_side(16.0, 12.0) - 12.0).abs() < f32::EPSILON);
    }
}
