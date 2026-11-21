use std::{iter, ops::Range};

use super::{UngroupedSlotLook, slot_marking::GroupedSlots};

pub(super) struct RowArrangement {
    pub(super) placement_order: Vec<usize>,
    pub(super) gaps_after: Vec<f32>,
}

struct PlacedChild {
    child_index: usize,
    gap_after: f32,
}

struct RowUnit {
    slots: Range<usize>,
    placed_children: Vec<PlacedChild>,
}

impl RowUnit {
    fn enclosed_group(heading_index: usize, grouped: &GroupedSlots, spacing: f32) -> Self {
        let thickness = grouped.tile_inset.thickness;
        let heading = PlacedChild {
            child_index: heading_index,
            gap_after: thickness,
        };
        let last_slot = grouped.positions.end - 1;
        let slots = grouped.positions.clone().map(|slot_position| PlacedChild {
            child_index: slot_position,
            gap_after: if slot_position == last_slot {
                thickness
            } else {
                spacing + thickness
            },
        });
        Self {
            slots: grouped.positions.clone(),
            placed_children: iter::once(heading).chain(slots).collect(),
        }
    }

    fn spaced_slots(slots: Range<usize>, spacing: f32) -> Self {
        let last_slot = slots.end - 1;
        Self {
            placed_children: slots
                .clone()
                .map(|slot_position| PlacedChild {
                    child_index: slot_position,
                    gap_after: if slot_position == last_slot {
                        0.0
                    } else {
                        spacing
                    },
                })
                .collect(),
            slots,
        }
    }
}

fn row_units(
    slot_count: usize,
    grouped_slots: &[GroupedSlots],
    ungrouped_look: &UngroupedSlotLook<'_>,
    spacing: f32,
) -> Vec<RowUnit> {
    let mut row_units: Vec<RowUnit> = Vec::new();
    let mut unit_start = 0;
    while unit_start < slot_count {
        let starting_group = grouped_slots
            .iter()
            .enumerate()
            .find(|(_, grouped)| grouped.positions.start == unit_start);
        let row_unit = match starting_group {
            Some((group_index, grouped)) => RowUnit::enclosed_group(
                RowArrangement::heading_index(slot_count, group_index),
                grouped,
                spacing,
            ),
            None => match ungrouped_look {
                UngroupedSlotLook::RestingTile { .. } => {
                    RowUnit::spaced_slots(unit_start..unit_start + 1, spacing)
                }
                UngroupedSlotLook::EdgeBand => {
                    let next_group_start = grouped_slots
                        .iter()
                        .map(|grouped| grouped.positions.start)
                        .filter(|group_start| *group_start > unit_start)
                        .min()
                        .unwrap_or(slot_count);
                    RowUnit::spaced_slots(unit_start..next_group_start, spacing)
                }
            },
        };
        unit_start = row_unit.slots.end;
        row_units.push(row_unit);
    }
    row_units
}

impl RowArrangement {
    pub(super) fn heading_index(slot_count: usize, group_index: usize) -> usize {
        slot_count + group_index
    }

    pub(super) fn interleave_group_headings(
        slot_count: usize,
        grouped_slots: &[GroupedSlots],
        ungrouped_look: &UngroupedSlotLook<'_>,
        spacing: f32,
        group_gap: f32,
    ) -> Self {
        let row_units = row_units(slot_count, grouped_slots, ungrouped_look, spacing);
        let mut placement_order = Vec::with_capacity(slot_count + grouped_slots.len());
        let mut gaps_after = vec![0.0; slot_count + grouped_slots.len()];
        for placed_child in row_units
            .iter()
            .flat_map(|row_unit| &row_unit.placed_children)
        {
            placement_order.push(placed_child.child_index);
            gaps_after[placed_child.child_index] = placed_child.gap_after;
        }
        for leading_unit in row_units.iter().rev().skip(1) {
            if let Some(unit_end) = leading_unit.placed_children.last() {
                gaps_after[unit_end.child_index] += group_gap;
            }
        }
        Self {
            placement_order,
            gaps_after,
        }
    }
}

#[cfg(test)]
mod tests {
    use iced::Color;

    use super::RowArrangement;
    use crate::drag_reorder::{
        reorderable_list::{UngroupedSlotLook, slot_marking::GroupedSlots},
        slot_group::SlotTileInset,
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
        let row_arrangement = RowArrangement::interleave_group_headings(
            4,
            &grouped_slots,
            &UngroupedSlotLook::EdgeBand,
            0.0,
            4.0,
        );
        assert_eq!(row_arrangement.placement_order, [0, 4, 1, 2, 3]);
    }

    #[test]
    fn slots_inside_a_group_are_one_enclosure_stroke_apart() {
        let grouped_slots = [GroupedSlots {
            positions: 0..2,
            tile_inset: TILE_INSET,
        }];
        let row_arrangement = RowArrangement::interleave_group_headings(
            4,
            &grouped_slots,
            &UngroupedSlotLook::EdgeBand,
            0.0,
            4.0,
        );
        assert_eq!(row_arrangement.gaps_after, [3.0, 7.0, 0.0, 0.0, 3.0]);
    }

    #[test]
    fn spacing_is_dropped_after_the_last_slot() {
        let row_arrangement = RowArrangement::interleave_group_headings(
            3,
            &[],
            &UngroupedSlotLook::EdgeBand,
            5.0,
            4.0,
        );
        assert_eq!(row_arrangement.gaps_after, [5.0, 5.0, 0.0]);
    }

    #[test]
    fn a_lone_tile_keeps_the_group_gap_on_both_sides() {
        let grouped_slots = [
            GroupedSlots {
                positions: 0..1,
                tile_inset: TILE_INSET,
            },
            GroupedSlots {
                positions: 2..3,
                tile_inset: TILE_INSET,
            },
        ];
        let resting_tile_look = UngroupedSlotLook::RestingTile {
            tile_inset: TILE_INSET,
            resting_color: Box::new(|_| Color::BLACK),
        };
        let row_arrangement = RowArrangement::interleave_group_headings(
            3,
            &grouped_slots,
            &resting_tile_look,
            0.0,
            4.0,
        );
        assert_eq!(row_arrangement.placement_order, [3, 0, 1, 4, 2]);
        assert_eq!(row_arrangement.gaps_after, [7.0, 4.0, 3.0, 3.0, 3.0]);
    }
}
