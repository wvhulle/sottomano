use super::window_row_presentation::{ArrangedWindowRows, WindowSlotPresentation};

pub(super) fn arranged_flat_rows<'a>(window_count: usize) -> ArrangedWindowRows<'a> {
    ArrangedWindowRows {
        groups: Vec::new(),
        presentations: vec![WindowSlotPresentation::Flat; window_count],
    }
}

#[cfg(test)]
mod tests {
    use super::{WindowSlotPresentation, arranged_flat_rows};

    #[test]
    fn flat_rows_have_no_groups_and_present_every_window_alone() {
        let arranged_rows = arranged_flat_rows(3);
        assert!(arranged_rows.groups.is_empty());
        assert_eq!(
            arranged_rows.presentations,
            [WindowSlotPresentation::Flat; 3]
        );
    }
}
