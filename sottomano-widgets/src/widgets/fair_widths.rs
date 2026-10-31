#[allow(clippy::cast_precision_loss)]
pub(crate) fn fair_widths(natural_widths: &[f32], room: f32) -> Vec<f32> {
    let mut narrowest_first: Vec<usize> = (0..natural_widths.len()).collect();
    narrowest_first.sort_by(|left, right| natural_widths[*left].total_cmp(&natural_widths[*right]));
    let mut allotted = vec![0.0; natural_widths.len()];
    narrowest_first
        .iter()
        .enumerate()
        .fold(room.max(0.0), |remaining, (handed_out, index)| {
            let still_waiting = (natural_widths.len() - handed_out) as f32;
            let width = natural_widths[*index].min(remaining / still_waiting);
            allotted[*index] = width;
            remaining - width
        });
    allotted
}

#[cfg(test)]
mod tests {
    use super::fair_widths;

    #[test]
    fn columns_that_fit_keep_their_natural_width() {
        assert_eq!(fair_widths(&[200.0, 300.0], 600.0), [200.0, 300.0]);
    }

    #[test]
    fn a_narrow_column_leaves_its_unused_share_to_the_wide_one() {
        assert_eq!(fair_widths(&[100.0, 900.0], 600.0), [100.0, 500.0]);
    }

    #[test]
    fn two_wide_columns_split_the_room_evenly() {
        assert_eq!(fair_widths(&[610.0, 1000.0], 700.0), [350.0, 350.0]);
    }
}
