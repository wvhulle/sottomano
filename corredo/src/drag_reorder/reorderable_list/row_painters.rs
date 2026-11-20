use iced::Element;

use super::{
    SlotAlarm,
    alarm_tiles::{AlarmTile, AlarmTiles},
    group_enclosures::GroupEnclosures,
    hover_glow::HoverGlow,
    slot_underlay::SlotUnderlay,
    tile_joins::{GroupedSlots, SlotMarking},
    travelling_highlight::TravellingHighlight,
};
use crate::drag_reorder::slot_group::{SlotGroup, SlotTileInset};

pub(super) struct RowPainters<'a> {
    pub(super) alarm_tiles: AlarmTiles<'a>,
    pub(super) hover_glow: HoverGlow,
    pub(super) travelling_highlight: TravellingHighlight<'a>,
    pub(super) group_enclosures: GroupEnclosures<'a>,
}

impl<'a> RowPainters<'a> {
    pub(super) fn painted_under<Message: 'a>(
        self,
        sliding_row: impl Into<Element<'a, Message>>,
    ) -> Element<'a, Message> {
        let alarmed_row = SlotUnderlay::new(self.alarm_tiles, sliding_row);
        let highlighted_row = SlotUnderlay::new(self.travelling_highlight, alarmed_row);
        let glowing_row = SlotUnderlay::new(self.hover_glow, highlighted_row);
        SlotUnderlay::new(self.group_enclosures, glowing_row).into()
    }
}

pub(super) fn grouped_slots_of<Message>(groups: &[SlotGroup<'_, Message>]) -> Vec<GroupedSlots> {
    groups
        .iter()
        .map(|group| GroupedSlots {
            positions: group.positions.clone(),
            tile_inset: SlotTileInset {
                thickness: group.enclosure_thickness,
                enclosure_radius: group.enclosure_radius,
            },
        })
        .collect()
}

pub(super) fn alarm_tiles_of<'a>(
    alarms: Vec<SlotAlarm<'a>>,
    slot_marking: &SlotMarking<'_>,
) -> Vec<AlarmTile<'a>> {
    alarms
        .into_iter()
        .filter_map(|alarm| {
            Some(AlarmTile {
                slot_position: alarm.position,
                tile_inset: slot_marking.tile_inset_at(alarm.position)?,
                tile_neighbours: slot_marking.tile_neighbours(alarm.position),
                color: alarm.color,
            })
        })
        .collect()
}
