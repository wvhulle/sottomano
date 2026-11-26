use std::hash::Hash;

use iced::{Color, Element, Theme, border::Radius, time::Duration, widget::mouse_area};

use crate::orientation::Edge;

mod alarm_tiles;
mod edge_band_geometry;
mod group_enclosures;
mod hover_glow;
mod row_arrangement;
mod row_painters;
mod sliding_row;
mod slot_marking;
mod slot_underlay;
mod travelling_highlight;
mod ungrouped_tiles;

use alarm_tiles::AlarmTiles;
use group_enclosures::{EnclosedGroup, GroupEnclosures};
use hover_glow::HoverGlow;
use row_arrangement::RowArrangement;
use row_painters::{RowPainters, marked_alarm_tiles};
use sliding_row::SlidingRow;
use slot_marking::{GroupedSlots, SlotMarking};
use travelling_highlight::{HighlightMark, HighlightTarget, HighlightedSlot, TravellingHighlight};
use ungrouped_tiles::UngroupedTiles;

use crate::{
    drag_reorder::{
        DragActivity, DragEvent, DragView,
        slot_group::{SlotGroup, SlotTileInset},
        wheel_stepper::{WheelStep, WheelStepper},
    },
    theming::{ThemedColor, style_metrics::ACCENT_STROKE_PX},
};

const DEFAULT_INDICATOR_MOTION: Duration = Duration::from_millis(160);
const DEFAULT_GROUP_GAP_PX: f32 = 4.0;

pub enum UngroupedSlotLook<'view> {
    EdgeBand,
    RestingTile {
        tile_inset: SlotTileInset,
        resting_color: ThemedColor<'view>,
    },
}

impl UngroupedSlotLook<'_> {
    pub(super) fn tile_inset(&self) -> Option<SlotTileInset> {
        match self {
            UngroupedSlotLook::EdgeBand => None,
            UngroupedSlotLook::RestingTile { tile_inset, .. } => Some(*tile_inset),
        }
    }
}

pub fn reorderable_list<'view, Message: 'view, SlotKey: Copy + Eq + Hash + 'static>(
    slots: impl IntoIterator<Item = (SlotKey, Element<'view, Message>)>,
    edge: Edge,
) -> ReorderableList<'view, Message, SlotKey> {
    let (keys, slots) = slots.into_iter().unzip();
    ReorderableList {
        keys,
        slots,
        edge,
        spacing: 0.0,
        motion: DEFAULT_INDICATOR_MOTION,
        indicator_thickness: ACCENT_STROKE_PX,
        highlighting: SlotHighlighting::Unhighlighted,
        ungrouped_slot_look: UngroupedSlotLook::EdgeBand,
        alarms: Vec::new(),
        groups: Vec::new(),
        group_gap: DEFAULT_GROUP_GAP_PX,
        frame_corners: Radius::default(),
        list_exit: ListExit::Unreported,
        wheel_select: WheelSelect::Ignored,
    }
}

pub struct SlotAlarm<'view> {
    pub position: usize,
    pub color: ThemedColor<'view>,
}

enum SlotHighlighting<'view, SlotKey> {
    Unhighlighted,
    Highlighted {
        key: SlotKey,
        color: ThemedColor<'view>,
    },
}

impl<'view, SlotKey: PartialEq> SlotHighlighting<'view, SlotKey> {
    fn target_among(
        self,
        keys: &[SlotKey],
        slot_marks: &[HighlightMark],
    ) -> HighlightTarget<'view> {
        let SlotHighlighting::Highlighted { key, color } = self else {
            return HighlightTarget::Unhighlighted;
        };
        let Some(slot_position) = keys.iter().position(|slot_key| *slot_key == key) else {
            return HighlightTarget::Unhighlighted;
        };
        HighlightTarget::Slot(HighlightedSlot {
            slot_position,
            mark: slot_marks[slot_position],
            color,
        })
    }
}

enum ListExit<Message> {
    Unreported,
    Reported(Message),
}

pub struct ReorderableList<'view, Message, SlotKey> {
    keys: Vec<SlotKey>,
    slots: Vec<Element<'view, Message>>,
    edge: Edge,
    spacing: f32,
    motion: Duration,
    indicator_thickness: f32,
    highlighting: SlotHighlighting<'view, SlotKey>,
    ungrouped_slot_look: UngroupedSlotLook<'view>,
    alarms: Vec<SlotAlarm<'view>>,
    groups: Vec<SlotGroup<'view, Message>>,
    group_gap: f32,
    frame_corners: Radius,
    list_exit: ListExit<Message>,
    wheel_select: WheelSelect<'view, SlotKey, Message>,
}

enum WheelSelect<'view, SlotKey, Message> {
    Ignored,
    Selects(Box<dyn Fn(SlotKey) -> Message + 'view>),
}

impl<'view, Message, SlotKey> ReorderableList<'view, Message, SlotKey> {
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
        highlighted_key: SlotKey,
        color: impl Fn(&Theme) -> Color + 'view,
    ) -> Self {
        self.highlighting = SlotHighlighting::Highlighted {
            key: highlighted_key,
            color: Box::new(color),
        };
        self
    }

    #[must_use]
    pub fn ungrouped_slot_look(mut self, look: UngroupedSlotLook<'view>) -> Self {
        self.ungrouped_slot_look = look;
        self
    }

    #[must_use]
    pub fn alarms(mut self, alarms: Vec<SlotAlarm<'view>>) -> Self {
        self.alarms = alarms;
        self
    }

    #[must_use]
    pub fn groups(mut self, groups: Vec<SlotGroup<'view, Message>>) -> Self {
        self.groups = groups;
        self
    }

    #[must_use]
    pub fn frame_corners(mut self, frame_corners: Radius) -> Self {
        self.frame_corners = frame_corners;
        self
    }

    #[must_use]
    pub fn on_wheel_select(self, select: impl Fn(SlotKey) -> Message + 'view) -> Self {
        Self {
            wheel_select: WheelSelect::Selects(Box::new(select)),
            ..self
        }
    }

    #[must_use]
    pub fn draggable<SlotId: Copy + PartialEq, ListId, ClickPayload>(
        mut self,
        list: ListId,
        drag: &DragView<SlotId>,
        on_drag: impl Fn(DragEvent<SlotId, ListId, ClickPayload>) -> Message,
    ) -> Self {
        self.list_exit = match drag.drag {
            DragActivity::Dragging(_) => ListExit::Reported(on_drag(DragEvent::LeftList(list))),
            DragActivity::Idle => ListExit::Unreported,
        };
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum RowKey<SlotKey> {
    Slot(SlotKey),
    GroupHeading(SlotKey),
}

impl<'view, Message: Clone + 'view, SlotKey: Copy + Eq + Hash + 'static>
    ReorderableList<'view, Message, SlotKey>
{
    fn composed(self) -> Element<'view, Message> {
        let ReorderableList {
            keys,
            slots,
            edge,
            spacing,
            motion,
            indicator_thickness,
            highlighting,
            ungrouped_slot_look,
            alarms,
            groups,
            group_gap,
            frame_corners,
            list_exit,
            wheel_select,
        } = self;
        let pager_axis = edge.running_axis();
        let slot_count = keys.len();
        let groups = groups_within_slots(groups, slot_count);
        let grouped_slots = groups.iter().map(GroupedSlots::from).collect::<Vec<_>>();
        let slot_marking = SlotMarking {
            grouped_slots: &grouped_slots,
            ungrouped_look: &ungrouped_slot_look,
        };
        let row_arrangement = RowArrangement::interleave_group_headings(
            slot_count,
            &grouped_slots,
            &ungrouped_slot_look,
            spacing,
            group_gap,
        );
        let slot_marks =
            slot_marking.slot_marks(slot_count, edge, frame_corners, indicator_thickness);
        let mut highlight_target = highlighting.target_among(&keys, &slot_marks);
        let highlighted_position = match &highlight_target {
            HighlightTarget::Slot(highlighted) => Some(highlighted.slot_position),
            HighlightTarget::Unhighlighted => None,
        };
        let mut row_children: Vec<(RowKey<SlotKey>, Element<'view, Message>)> =
            keys.iter().copied().map(RowKey::Slot).zip(slots).collect();
        let mut enclosed_groups = Vec::with_capacity(groups.len());
        for (group_index, (group, grouped)) in groups.into_iter().zip(&grouped_slots).enumerate() {
            if let HighlightTarget::Slot(highlighted) = &mut highlight_target
                && group.positions.contains(&highlighted.slot_position)
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
                alarm_tiles: marked_alarm_tiles(alarms, &slot_marking),
            },
            hover_glow: HoverGlow {
                pager_axis,
                fade_duration: motion,
                slot_marks,
            },
            travelling_highlight: TravellingHighlight {
                pager_axis,
                travel_duration: motion,
                highlight_target,
            },
            group_enclosures: GroupEnclosures {
                pager_axis,
                enclosed_groups,
            },
            ungrouped_tiles: UngroupedTiles {
                pager_axis,
                slot_positions: slot_marking.ungrouped_positions(slot_count),
                ungrouped_look: ungrouped_slot_look,
            },
        }
        .painted_under(sliding_row);
        let departing_row = exiting_with(painted_row, list_exit);
        wheel_selecting(departing_row, wheel_select, keys, highlighted_position)
    }
}

fn groups_within_slots<Message>(
    groups: Vec<SlotGroup<'_, Message>>,
    slot_count: usize,
) -> Vec<SlotGroup<'_, Message>> {
    groups
        .into_iter()
        .filter(|group| !group.positions.is_empty() && group.positions.end <= slot_count)
        .collect()
}

fn exiting_with<'view, Message: Clone + 'view>(
    row: Element<'view, Message>,
    list_exit: ListExit<Message>,
) -> Element<'view, Message> {
    match list_exit {
        ListExit::Reported(left_list_message) => mouse_area(row).on_exit(left_list_message).into(),
        ListExit::Unreported => row,
    }
}

fn wheel_selecting<'view, Message: 'view, SlotKey: Copy + 'static>(
    row: Element<'view, Message>,
    wheel_select: WheelSelect<'view, SlotKey, Message>,
    keys: Vec<SlotKey>,
    highlighted_position: Option<usize>,
) -> Element<'view, Message> {
    let WheelSelect::Selects(select) = wheel_select else {
        return row;
    };
    WheelStepper::new(row, move |step| {
        let position = highlighted_position?;
        let neighbour = match step {
            WheelStep::Backward => position.checked_sub(1)?,
            WheelStep::Forward => position.checked_add(1)?,
        };
        keys.get(neighbour).map(|key| select(*key))
    })
    .into()
}

impl<'view, Message: Clone + 'view, SlotKey: Copy + Eq + Hash + 'static>
    From<ReorderableList<'view, Message, SlotKey>> for Element<'view, Message>
{
    fn from(reorderable_list: ReorderableList<'view, Message, SlotKey>) -> Self {
        reorderable_list.composed()
    }
}

#[cfg(test)]
mod wheel_select_tests;
