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
use row_painters::{RowPainters, alarm_tiles_of, grouped_slots_of};
use sliding_row::SlidingRow;
use slot_marking::SlotMarking;
use travelling_highlight::{HighlightedSlot, TravellingHighlight};
use ungrouped_tiles::UngroupedTiles;

use crate::{
    drag_reorder::{
        DragEvent, DragView,
        slot_group::{SlotGroup, SlotTileInset},
        wheel_stepper::{WheelStep, WheelStepper},
    },
    theming::{ThemedColor, style_metrics::ACCENT_STROKE_PX},
};

const DEFAULT_INDICATOR_MOTION: Duration = Duration::from_millis(160);
const DEFAULT_GROUP_GAP_PX: f32 = 4.0;

pub enum UngroupedSlotLook<'a> {
    EdgeBand,
    RestingTile {
        tile_inset: SlotTileInset,
        resting_color: ThemedColor<'a>,
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

pub fn reorderable_list<'a, Message: 'a, SlotKey: Copy + Eq + Hash + 'static>(
    slots: impl IntoIterator<Item = (SlotKey, Element<'a, Message>)>,
    edge: Edge,
) -> ReorderableList<'a, Message, SlotKey> {
    let (keys, slots) = slots.into_iter().unzip();
    ReorderableList {
        keys,
        slots,
        edge,
        spacing: 0.0,
        motion: DEFAULT_INDICATOR_MOTION,
        indicator_thickness: ACCENT_STROKE_PX,
        highlight: None,
        ungrouped_slot_look: UngroupedSlotLook::EdgeBand,
        alarms: Vec::new(),
        groups: Vec::new(),
        group_gap: DEFAULT_GROUP_GAP_PX,
        frame_corners: Radius::default(),
        left_list_message: None,
        wheel_select: WheelSelect::Ignored,
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

pub struct ReorderableList<'a, Message, SlotKey> {
    keys: Vec<SlotKey>,
    slots: Vec<Element<'a, Message>>,
    edge: Edge,
    spacing: f32,
    motion: Duration,
    indicator_thickness: f32,
    highlight: Option<SlotHighlight<'a, SlotKey>>,
    ungrouped_slot_look: UngroupedSlotLook<'a>,
    alarms: Vec<SlotAlarm<'a>>,
    groups: Vec<SlotGroup<'a, Message>>,
    group_gap: f32,
    frame_corners: Radius,
    left_list_message: Option<Message>,
    wheel_select: WheelSelect<'a, SlotKey, Message>,
}

enum WheelSelect<'a, SlotKey, Message> {
    Ignored,
    Selects(Box<dyn Fn(SlotKey) -> Message + 'a>),
}

impl<'a, Message, SlotKey> ReorderableList<'a, Message, SlotKey> {
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
    pub fn ungrouped_slot_look(mut self, look: UngroupedSlotLook<'a>) -> Self {
        self.ungrouped_slot_look = look;
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
    pub fn on_wheel_select(self, select: impl Fn(SlotKey) -> Message + 'a) -> Self {
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
        self.left_list_message = drag.drag.map(|_| on_drag(DragEvent::LeftList(list)));
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum RowKey<SlotKey> {
    Slot(SlotKey),
    GroupHeading(SlotKey),
}

impl<'a, Message: Clone + 'a, SlotKey: Copy + Eq + Hash + 'static>
    ReorderableList<'a, Message, SlotKey>
{
    fn composed(self) -> Element<'a, Message> {
        let ReorderableList {
            keys,
            slots,
            edge,
            spacing,
            motion,
            indicator_thickness,
            highlight,
            ungrouped_slot_look,
            alarms,
            groups,
            group_gap,
            frame_corners,
            left_list_message,
            wheel_select,
        } = self;
        let pager_axis = edge.running_axis();
        let slot_count = keys.len();
        let groups = groups_within_slots(groups, slot_count);
        let grouped_slots = grouped_slots_of(&groups);
        let highlighted_position = highlight
            .as_ref()
            .and_then(|slot_highlight| keys.iter().position(|key| *key == slot_highlight.key));
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
        let mut highlighted_slot =
            highlight
                .zip(highlighted_position)
                .map(|(slot_highlight, slot_position)| HighlightedSlot {
                    slot_position,
                    mark: slot_marks[slot_position],
                    color: slot_highlight.color,
                });
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
                alarm_tiles: alarm_tiles_of(alarms, &slot_marking),
            },
            hover_glow: HoverGlow {
                pager_axis,
                fade_duration: motion,
                slot_marks,
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
            ungrouped_tiles: UngroupedTiles {
                pager_axis,
                slot_positions: slot_marking.ungrouped_positions(slot_count),
                ungrouped_look: ungrouped_slot_look,
            },
        }
        .painted_under(sliding_row);
        let departing_row = exiting_with(painted_row, left_list_message);
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

fn exiting_with<'a, Message: Clone + 'a>(
    row: Element<'a, Message>,
    left_list_message: Option<Message>,
) -> Element<'a, Message> {
    match left_list_message {
        Some(left_list_message) => mouse_area(row).on_exit(left_list_message).into(),
        None => row,
    }
}

fn wheel_selecting<'a, Message: 'a, SlotKey: Copy + 'static>(
    row: Element<'a, Message>,
    wheel_select: WheelSelect<'a, SlotKey, Message>,
    keys: Vec<SlotKey>,
    highlighted_position: Option<usize>,
) -> Element<'a, Message> {
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

impl<'a, Message: Clone + 'a, SlotKey: Copy + Eq + Hash + 'static>
    From<ReorderableList<'a, Message, SlotKey>> for Element<'a, Message>
{
    fn from(reorderable_list: ReorderableList<'a, Message, SlotKey>) -> Self {
        reorderable_list.composed()
    }
}

#[cfg(test)]
mod tests {
    use iced::{
        Color, Element, Event, Point, Settings, Size,
        mouse::{Event as MouseEvent, ScrollDelta},
        widget::text,
    };
    use iced_test::Simulator;

    use super::reorderable_list;
    use crate::orientation::Edge;

    const SLOT_KEYS: [u8; 3] = [10, 20, 30];
    const WHEEL_UP: ScrollDelta = ScrollDelta::Lines { x: 0.0, y: 1.0 };
    const WHEEL_DOWN: ScrollDelta = ScrollDelta::Lines { x: 0.0, y: -1.0 };

    fn selected_after_wheel(highlighted: u8, delta: ScrollDelta) -> Vec<u8> {
        let slots = SLOT_KEYS
            .into_iter()
            .map(|key| -> (u8, Element<'_, u8>) { (key, text(format!("slot {key}")).into()) });
        let list: Element<'_, u8> = reorderable_list(slots, Edge::Left)
            .highlight(Some(highlighted), |_| Color::WHITE)
            .on_wheel_select(|key| key)
            .into();
        let mut simulator =
            Simulator::with_size(Settings::default(), Size::new(200.0, 200.0), list);
        let pointer = simulator
            .find("slot 20")
            .expect("the middle slot is laid out")
            .bounds()
            .center();
        simulator.point_at(pointer);
        let _moved = simulator.simulate([Event::Mouse(MouseEvent::CursorMoved {
            position: Point::new(pointer.x, pointer.y),
        })]);
        let _wheeled = simulator.simulate([Event::Mouse(MouseEvent::WheelScrolled { delta })]);
        simulator.into_messages().collect()
    }

    #[test]
    fn the_wheel_selects_the_neighbour_of_the_highlighted_slot() {
        assert_eq!(selected_after_wheel(20, WHEEL_UP), [10]);
        assert_eq!(selected_after_wheel(20, WHEEL_DOWN), [30]);
    }

    #[test]
    fn the_wheel_stops_at_either_end() {
        assert!(selected_after_wheel(10, WHEEL_UP).is_empty());
        assert!(selected_after_wheel(30, WHEEL_DOWN).is_empty());
    }
}
