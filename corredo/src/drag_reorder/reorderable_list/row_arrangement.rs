use super::tile_joins::GroupedSlots;

pub(super) struct RowArrangement {
    pub(super) placement_order: Vec<usize>,
    pub(super) gaps_after: Vec<f32>,
}

impl RowArrangement {
    pub(super) fn heading_index(slot_count: usize, group_index: usize) -> usize {
        slot_count + group_index
    }

    pub(super) fn of(
        slot_count: usize,
        grouped_slots: &[GroupedSlots],
        spacing: f32,
        group_gap: f32,
    ) -> Self {
        let child_count = slot_count + grouped_slots.len();
        let mut placement_order = Vec::with_capacity(child_count);
        let mut gaps_after = vec![0.0; child_count];
        for slot_position in 0..slot_count {
            let enclosing_group = grouped_slots
                .iter()
                .enumerate()
                .find(|(_, grouped)| grouped.positions.contains(&slot_position));
            if let Some((group_index, grouped)) = enclosing_group
                && grouped.positions.start == slot_position
            {
                let heading_index = Self::heading_index(slot_count, group_index);
                placement_order.push(heading_index);
                gaps_after[heading_index] = grouped.tile_inset.thickness;
            }
            placement_order.push(slot_position);
            gaps_after[slot_position] = spacing
                + match enclosing_group {
                    Some((_, grouped)) if grouped.positions.end == slot_position + 1 => {
                        grouped.tile_inset.thickness + group_gap
                    }
                    Some((_, grouped)) => grouped.tile_inset.thickness,
                    None => 0.0,
                };
        }
        if let Some(&last_placed) = placement_order.last() {
            gaps_after[last_placed] = (gaps_after[last_placed] - spacing).max(0.0);
        }
        Self {
            placement_order,
            gaps_after,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RowArrangement;
    use crate::drag_reorder::{
        reorderable_list::tile_joins::GroupedSlots, slot_group::SlotTileInset,
    };

    const TILE_INSET: SlotTileInset = SlotTileInset {
        thickness: 3.0,
        enclosure_radius: 8.0,
    };

    #[test]
    fn a_heading_is_placed_before_its_group_but_listed_after_every_slot() {
        let grouped_slots = [GroupedSlots {
            positions: 1..3,
            tile_inset: TILE_INSET,
        }];
        let row_arrangement = RowArrangement::of(4, &grouped_slots, 0.0, 4.0);
        assert_eq!(row_arrangement.placement_order, [0, 4, 1, 2, 3]);
    }

    #[test]
    fn slots_inside_a_group_are_one_enclosure_stroke_apart() {
        let grouped_slots = [GroupedSlots {
            positions: 0..2,
            tile_inset: TILE_INSET,
        }];
        let row_arrangement = RowArrangement::of(4, &grouped_slots, 0.0, 4.0);
        assert_eq!(row_arrangement.gaps_after, [3.0, 7.0, 0.0, 0.0, 3.0]);
    }

    #[test]
    fn spacing_is_dropped_after_the_last_slot() {
        let row_arrangement = RowArrangement::of(3, &[], 5.0, 4.0);
        assert_eq!(row_arrangement.gaps_after, [5.0, 5.0, 0.0]);
    }
}
