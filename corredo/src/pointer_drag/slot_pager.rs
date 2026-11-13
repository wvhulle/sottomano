use std::hash::Hash;

use iced::{Color, Element, Theme, border::Radius, time::Duration, widget::mouse_area};

mod alarm_tiles;
mod edge_band_geometry;
mod group_enclosures;
mod row_arrangement;
mod sliding_row;
mod slot_underlay;
mod tile_joins;
mod travelling_highlight;

use alarm_tiles::{AlarmTile, AlarmTiles};
use group_enclosures::{EnclosedGroup, GroupEnclosures};
use row_arrangement::RowArrangement;
use sliding_row::SlidingRow;
use slot_underlay::SlotUnderlay;
use tile_joins::{GroupedSlots, SlotMarking};
use travelling_highlight::{HighlightMark, HighlightedSlot, TravellingHighlight};

use crate::{
    list_end_corners::ListEndCorners,
    pointer_drag::{
        DragEvent, DragView,
        slot_group::{PagerAxis, SlotGroup, SlotTileInset},
    },
    shared_metrics::ACCENT_STROKE_PX,
    theme_colors::ThemedColor,
};

const DEFAULT_INDICATOR_MOTION: Duration = Duration::from_millis(160);
const DEFAULT_GROUP_GAP_PX: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagerEdge {
    Top,
    Bottom,
    Left,
    Right,
}

impl From<PagerEdge> for PagerAxis {
    fn from(edge: PagerEdge) -> Self {
        match edge {
            PagerEdge::Left | PagerEdge::Right => PagerAxis::Vertical,
            PagerEdge::Top | PagerEdge::Bottom => PagerAxis::Horizontal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UngroupedHighlightLook {
    EdgeBand,
    AccentTile(SlotTileInset),
}

pub fn slot_pager<'a, Message: 'a, SlotKey: Copy + Eq + Hash + 'static>(
    slots: impl IntoIterator<Item = (SlotKey, Element<'a, Message>)>,
    edge: PagerEdge,
) -> SlotPager<'a, Message, SlotKey> {
    let (keys, slots) = slots.into_iter().unzip();
    SlotPager {
        keys,
        slots,
        edge,
        spacing: 0.0,
        motion: DEFAULT_INDICATOR_MOTION,
        indicator_thickness: ACCENT_STROKE_PX,
        highlight: None,
        ungrouped_highlight_look: UngroupedHighlightLook::EdgeBand,
        alarms: Vec::new(),
        groups: Vec::new(),
        group_gap: DEFAULT_GROUP_GAP_PX,
        frame_corners: Radius::default(),
        left_list_message: None,
    }
}

pub struct SlotAlarm<'a> {
    pub position: usize,
    pub color: ThemedColor<'a>,
}

struct SlotHighlight<'a, SlotKey> {
    key: SlotKey,
    color: ThemedColor<'a>,
}

pub struct SlotPager<'a, Message, SlotKey> {
    keys: Vec<SlotKey>,
    slots: Vec<Element<'a, Message>>,
    edge: PagerEdge,
    spacing: f32,
    motion: Duration,
    indicator_thickness: f32,
    highlight: Option<SlotHighlight<'a, SlotKey>>,
    ungrouped_highlight_look: UngroupedHighlightLook,
    alarms: Vec<SlotAlarm<'a>>,
    groups: Vec<SlotGroup<'a, Message>>,
    group_gap: f32,
    frame_corners: Radius,
    left_list_message: Option<Message>,
}

impl<'a, Message, SlotKey> SlotPager<'a, Message, SlotKey> {
    #[must_use]
    pub fn indicator_motion(self, motion: Duration) -> Self {
        Self { motion, ..self }
    }

    #[must_use]
    pub fn group_gap(self, group_gap: f32) -> Self {
        Self { group_gap, ..self }
    }

    #[must_use]
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    #[must_use]
    pub fn highlight(
        mut self,
        highlighted_key: Option<SlotKey>,
        color: impl Fn(&Theme) -> Color + 'a,
    ) -> Self {
        self.highlight = highlighted_key.map(|key| SlotHighlight {
            key,
            color: Box::new(color),
        });
        self
    }

    #[must_use]
    pub fn ungrouped_highlight_look(mut self, look: UngroupedHighlightLook) -> Self {
        self.ungrouped_highlight_look = look;
        self
    }

    #[must_use]
    pub fn alarms(mut self, alarms: Vec<SlotAlarm<'a>>) -> Self {
        self.alarms = alarms;
        self
    }

    #[must_use]
    pub fn groups(mut self, groups: Vec<SlotGroup<'a, Message>>) -> Self {
        self.groups = groups;
        self
    }

    #[must_use]
    pub fn frame_corners(mut self, frame_corners: Radius) -> Self {
        self.frame_corners = frame_corners;
        self
    }

    #[must_use]
    pub fn draggable<SlotId: Copy + PartialEq, ListId, ClickPayload>(
        mut self,
        list: ListId,
        drag: &DragView<SlotId>,
        on_drag: impl Fn(DragEvent<SlotId, ListId, ClickPayload>) -> Message,
    ) -> Self {
        self.left_list_message = drag.drag.map(|_| on_drag(DragEvent::LeftList(list)));
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum RowKey<SlotKey> {
    Slot(SlotKey),
    GroupHeading(SlotKey),
}

impl<'a, Message: Clone + 'a, SlotKey: Copy + Eq + Hash + 'static> SlotPager<'a, Message, SlotKey> {
    fn composed(self) -> Element<'a, Message> {
        let SlotPager {
            keys,
            slots,
            edge,
            spacing,
            motion,
            indicator_thickness,
            highlight,
            ungrouped_highlight_look,
            alarms,
            groups,
            group_gap,
            frame_corners,
            left_list_message,
        } = self;
        let pager_axis = PagerAxis::from(edge);
        let slot_count = keys.len();
        let groups: Vec<_> = groups
            .into_iter()
            .filter(|group| !group.positions.is_empty() && group.positions.end <= slot_count)
            .collect();
        let grouped_slots = grouped_slots_of(&groups);
        let highlighted_position = highlight
            .as_ref()
            .and_then(|slot_highlight| keys.iter().position(|key| *key == slot_highlight.key));
        let slot_marking = SlotMarking {
            grouped_slots: &grouped_slots,
            ungrouped_look: ungrouped_highlight_look,
            marked_positions: highlighted_position
                .into_iter()
                .chain(alarms.iter().map(|alarm| alarm.position))
                .collect(),
        };
        let row_arrangement = RowArrangement::of(slot_count, &grouped_slots, spacing, group_gap);
        let mut highlighted_slot =
            highlight
                .zip(highlighted_position)
                .map(|(slot_highlight, slot_position)| HighlightedSlot {
                    slot_position,
                    mark: highlight_mark(
                        &slot_marking,
                        slot_position,
                        HighlightMark::EdgeBand {
                            edge,
                            corners: ListEndCorners::new(slot_count, frame_corners)
                                .radius_at(slot_position),
                            thickness: indicator_thickness,
                        },
                    ),
                    color: slot_highlight.color,
                });
        let alarm_tiles = alarm_tiles_of(alarms, &slot_marking);
        let mut row_children: Vec<(RowKey<SlotKey>, Element<'a, Message>)> =
            keys.iter().copied().map(RowKey::Slot).zip(slots).collect();
        let mut enclosed_groups = Vec::with_capacity(groups.len());
        for (group_index, (group, grouped)) in groups.into_iter().zip(&grouped_slots).enumerate() {
            if let Some(highlighted) = highlighted_slot
                .as_mut()
                .filter(|highlighted| group.positions.contains(&highlighted.slot_position))
            {
                highlighted.color = group.highlight_color;
            }
            row_children.push((
                RowKey::GroupHeading(keys[group.positions.start]),
                group.heading,
            ));
            enclosed_groups.push(EnclosedGroup {
                heading_index: RowArrangement::heading_index(slot_count, group_index),
                slot_positions: group.positions,
                tile_inset: grouped.tile_inset,
                enclosure_color: group.enclosure_color,
                inner_color: group.inner_color,
            });
        }
        let sliding_row = SlidingRow::new(row_children, pager_axis, motion)
            .placed_in_order(row_arrangement.placement_order)
            .gaps_after(row_arrangement.gaps_after);
        let painted_row = RowPainters {
            alarm_tiles: AlarmTiles {
                pager_axis,
                alarm_tiles,
            },
            travelling_highlight: TravellingHighlight {
                pager_axis,
                travel_duration: motion,
                highlighted_slot,
            },
            group_enclosures: GroupEnclosures {
                pager_axis,
                enclosed_groups,
            },
        }
        .painted_under(sliding_row);
        match left_list_message {
            Some(left_list_message) => mouse_area(painted_row).on_exit(left_list_message).into(),
            None => painted_row,
        }
    }
}

struct RowPainters<'a> {
    alarm_tiles: AlarmTiles<'a>,
    travelling_highlight: TravellingHighlight<'a>,
    group_enclosures: GroupEnclosures<'a>,
}

impl<'a> RowPainters<'a> {
    fn painted_under<Message: 'a>(
        self,
        sliding_row: impl Into<Element<'a, Message>>,
    ) -> Element<'a, Message> {
        let alarmed_row = SlotUnderlay::new(self.alarm_tiles, sliding_row);
        let highlighted_row = SlotUnderlay::new(self.travelling_highlight, alarmed_row);
        SlotUnderlay::new(self.group_enclosures, highlighted_row).into()
    }
}

fn grouped_slots_of<Message>(groups: &[SlotGroup<'_, Message>]) -> Vec<GroupedSlots> {
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

fn highlight_mark(
    slot_marking: &SlotMarking<'_>,
    slot_position: usize,
    edge_band_mark: HighlightMark,
) -> HighlightMark {
    match slot_marking.tile_inset_at(slot_position) {
        Some(tile_inset) => HighlightMark::Tile {
            tile_inset,
            tile_neighbours: slot_marking.tile_neighbours(slot_position),
        },
        None => edge_band_mark,
    }
}

fn alarm_tiles_of<'a>(
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

impl<'a, Message: Clone + 'a, SlotKey: Copy + Eq + Hash + 'static>
    From<SlotPager<'a, Message, SlotKey>> for Element<'a, Message>
{
    fn from(slot_pager: SlotPager<'a, Message, SlotKey>) -> Self {
        slot_pager.composed()
    }
}
