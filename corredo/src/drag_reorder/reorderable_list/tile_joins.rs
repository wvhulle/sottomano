use std::ops::Range;

use super::UngroupedHighlightLook;
use crate::drag_reorder::slot_group::{SlotTileInset, TileNeighbours};

pub(super) struct GroupedSlots {
    pub(super) positions: Range<usize>,
    pub(super) tile_inset: SlotTileInset,
}

pub(super) struct SlotMarking<'groups> {
    pub(super) grouped_slots: &'groups [GroupedSlots],
    pub(super) ungrouped_look: UngroupedHighlightLook,
    pub(super) marked_positions: Vec<usize>,
}

impl SlotMarking<'_> {
    fn grouped_slots_at(&self, slot_position: usize) -> Option<&GroupedSlots> {
        self.grouped_slots
            .iter()
            .find(|grouped| grouped.positions.contains(&slot_position))
    }

    pub(super) fn tile_inset_at(&self, slot_position: usize) -> Option<SlotTileInset> {
        match self.grouped_slots_at(slot_position) {
            Some(grouped) => Some(grouped.tile_inset),
            None => match self.ungrouped_look {
                UngroupedHighlightLook::EdgeBand => None,
                UngroupedHighlightLook::AccentTile(tile_inset) => Some(tile_inset),
            },
        }
    }

    fn joins_ungrouped_tile(&self, slot_position: usize, neighbour_position: usize) -> bool {
        matches!(self.ungrouped_look, UngroupedHighlightLook::AccentTile(_))
            && self.marked_positions.contains(&neighbour_position)
            && self.grouped_slots_at(slot_position).is_none()
            && self.grouped_slots_at(neighbour_position).is_none()
    }

    pub(super) fn tile_neighbours(&self, slot_position: usize) -> TileNeighbours {
        TileNeighbours {
            leading: slot_position
                .checked_sub(1)
                .is_some_and(|leading_position| {
                    self.joins_ungrouped_tile(slot_position, leading_position)
                }),
            trailing: self.joins_ungrouped_tile(slot_position, slot_position + 1),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use super::{GroupedSlots, SlotMarking};
    use crate::drag_reorder::{
        reorderable_list::UngroupedHighlightLook,
        slot_group::{SlotTileInset, TileNeighbours},
    };

    const TILE_INSET: SlotTileInset = SlotTileInset {
        thickness: 3.0,
        enclosure_radius: 8.0,
    };
    const ACCENT_TILE_LOOK: UngroupedHighlightLook = UngroupedHighlightLook::AccentTile(TILE_INSET);
    const UNJOINED: TileNeighbours = TileNeighbours {
        leading: false,
        trailing: false,
    };

    fn grouped_over(positions: Range<usize>) -> GroupedSlots {
        GroupedSlots {
            positions,
            tile_inset: TILE_INSET,
        }
    }

    #[test]
    fn highlight_and_alarm_in_one_group_keep_their_own_cells() {
        let grouped_slots = [grouped_over(0..4)];
        let slot_marking = SlotMarking {
            grouped_slots: &grouped_slots,
            ungrouped_look: UngroupedHighlightLook::EdgeBand,
            marked_positions: vec![1, 2],
        };
        assert_eq!(slot_marking.tile_neighbours(1), UNJOINED);
        assert_eq!(slot_marking.tile_neighbours(2), UNJOINED);
    }

    #[test]
    fn neighbouring_ungrouped_accent_tiles_join() {
        let slot_marking = SlotMarking {
            grouped_slots: &[],
            ungrouped_look: ACCENT_TILE_LOOK,
            marked_positions: vec![0, 1],
        };
        assert_eq!(
            slot_marking.tile_neighbours(0),
            TileNeighbours {
                leading: false,
                trailing: true,
            }
        );
    }

    #[test]
    fn grouped_tile_stays_apart_from_ungrouped_tile() {
        let grouped_slots = [grouped_over(0..2)];
        let slot_marking = SlotMarking {
            grouped_slots: &grouped_slots,
            ungrouped_look: ACCENT_TILE_LOOK,
            marked_positions: vec![1, 2],
        };
        assert_eq!(slot_marking.tile_neighbours(1), UNJOINED);
        assert_eq!(slot_marking.tile_neighbours(2), UNJOINED);
    }

    #[test]
    fn edge_band_look_never_joins_and_draws_no_tile() {
        let slot_marking = SlotMarking {
            grouped_slots: &[],
            ungrouped_look: UngroupedHighlightLook::EdgeBand,
            marked_positions: vec![0, 1],
        };
        assert_eq!(slot_marking.tile_neighbours(0), UNJOINED);
        assert_eq!(slot_marking.tile_inset_at(0), None);
    }

    #[test]
    fn a_grouped_slot_takes_its_group_inset() {
        let grouped_slots = [grouped_over(2..4)];
        let slot_marking = SlotMarking {
            grouped_slots: &grouped_slots,
            ungrouped_look: UngroupedHighlightLook::EdgeBand,
            marked_positions: Vec::new(),
        };
        assert_eq!(slot_marking.tile_inset_at(3), Some(TILE_INSET));
    }
}
