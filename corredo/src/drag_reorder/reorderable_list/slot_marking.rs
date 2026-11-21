use std::ops::Range;

use iced::border::Radius;

use super::{UngroupedSlotLook, travelling_highlight::HighlightMark};
use crate::{
    drag_reorder::{ListEndCorners, slot_group::SlotTileInset},
    orientation::Edge,
};

pub(super) struct GroupedSlots {
    pub(super) positions: Range<usize>,
    pub(super) tile_inset: SlotTileInset,
}

pub(super) struct SlotMarking<'groups> {
    pub(super) grouped_slots: &'groups [GroupedSlots],
    pub(super) ungrouped_look: &'groups UngroupedSlotLook<'groups>,
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
            None => self.ungrouped_look.tile_inset(),
        }
    }

    pub(super) fn ungrouped_positions(&self, slot_count: usize) -> Vec<usize> {
        (0..slot_count)
            .filter(|slot_position| self.grouped_slots_at(*slot_position).is_none())
            .collect()
    }

    pub(super) fn slot_marks(
        &self,
        slot_count: usize,
        edge: Edge,
        frame_corners: Radius,
        thickness: f32,
    ) -> Vec<HighlightMark> {
        let end_corners = ListEndCorners::new(slot_count, frame_corners);
        (0..slot_count)
            .map(|slot_position| match self.tile_inset_at(slot_position) {
                Some(tile_inset) => HighlightMark::Tile { tile_inset },
                None => HighlightMark::EdgeBand {
                    edge,
                    corners: end_corners.radius_at(slot_position),
                    thickness,
                },
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use iced::Color;

    use super::{GroupedSlots, SlotMarking};
    use crate::drag_reorder::{reorderable_list::UngroupedSlotLook, slot_group::SlotTileInset};

    const TILE_INSET: SlotTileInset = SlotTileInset {
        thickness: 3.0,
        enclosure_radius: 8.0,
    };

    fn resting_tile_look() -> UngroupedSlotLook<'static> {
        UngroupedSlotLook::RestingTile {
            tile_inset: TILE_INSET,
            resting_color: Box::new(|_| Color::BLACK),
        }
    }

    fn grouped_over(positions: Range<usize>) -> GroupedSlots {
        GroupedSlots {
            positions,
            tile_inset: TILE_INSET,
        }
    }

    #[test]
    fn edge_band_look_draws_no_tile_outside_groups() {
        let slot_marking = SlotMarking {
            grouped_slots: &[],
            ungrouped_look: &UngroupedSlotLook::EdgeBand,
        };
        assert_eq!(slot_marking.tile_inset_at(0), None);
    }

    #[test]
    fn a_grouped_slot_takes_its_group_inset() {
        let grouped_slots = [grouped_over(2..4)];
        let slot_marking = SlotMarking {
            grouped_slots: &grouped_slots,
            ungrouped_look: &UngroupedSlotLook::EdgeBand,
        };
        assert_eq!(slot_marking.tile_inset_at(3), Some(TILE_INSET));
    }

    #[test]
    fn slots_around_and_between_groups_count_as_ungrouped() {
        let grouped_slots = [grouped_over(1..3), grouped_over(4..5)];
        let slot_marking = SlotMarking {
            grouped_slots: &grouped_slots,
            ungrouped_look: &resting_tile_look(),
        };
        assert_eq!(slot_marking.ungrouped_positions(7), [0, 3, 5, 6]);
    }
}
