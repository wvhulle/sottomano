use iced::{Element, Rectangle, Theme, mouse::Cursor, time::Instant};

use super::{
    SlotAlarm,
    alarm_tiles::{AlarmTile, AlarmTiles},
    group_enclosures::GroupEnclosures,
    hover_glow::{HoverGlow, SlotGlow},
    slot_underlay::{PaintedRow, SlotPainter, SlotUnderlay},
    tile_joins::{GroupedSlots, SlotMarking},
    travelling_highlight::{HighlightTravelState, TravellingHighlight},
};
use crate::{
    drag_reorder::slot_group::{SlotGroup, SlotTileInset},
    transitions::MotionState,
};

pub(super) struct RowPainters<'a> {
    pub(super) alarm_tiles: AlarmTiles<'a>,
    pub(super) hover_glow: HoverGlow,
    pub(super) travelling_highlight: TravellingHighlight<'a>,
    pub(super) group_enclosures: GroupEnclosures<'a>,
}

#[derive(Default)]
pub(super) struct RowPainterStates {
    slot_glows: Vec<SlotGlow>,
    highlight_travel: HighlightTravelState,
}

fn painted_with<'state, PainterState>(
    painted_row: &PaintedRow<'state, RowPainterStates>,
    painter_state: &'state PainterState,
) -> PaintedRow<'state, PainterState> {
    PaintedRow {
        painter_state,
        child_bounds: painted_row.child_bounds,
        row_bounds: painted_row.row_bounds,
    }
}

impl<'a> RowPainters<'a> {
    pub(super) fn painted_under<Message: 'a>(
        self,
        sliding_row: impl Into<Element<'a, Message>>,
    ) -> Element<'a, Message> {
        SlotUnderlay::new(self, sliding_row).into()
    }
}

impl SlotPainter for RowPainters<'_> {
    type PainterState = RowPainterStates;

    fn advance(
        &self,
        painter_states: &mut RowPainterStates,
        child_bounds: &[Rectangle],
        cursor: Cursor,
        now: Instant,
    ) -> MotionState {
        let glow_motion =
            self.hover_glow
                .advance(&mut painter_states.slot_glows, child_bounds, cursor, now);
        let highlight_motion = self.travelling_highlight.advance(
            &mut painter_states.highlight_travel,
            child_bounds,
            cursor,
            now,
        );
        glow_motion.moving_with(highlight_motion)
    }

    fn pointer_motion(
        &self,
        painter_states: &RowPainterStates,
        child_bounds: &[Rectangle],
        cursor: Cursor,
    ) -> MotionState {
        self.hover_glow
            .pointer_motion(&painter_states.slot_glows, child_bounds, cursor)
            .moving_with(self.travelling_highlight.pointer_motion(
                &painter_states.highlight_travel,
                child_bounds,
                cursor,
            ))
    }

    fn draw_under(
        &self,
        painted_row: PaintedRow<'_, RowPainterStates>,
        renderer: &mut iced::Renderer,
        theme: &Theme,
    ) {
        self.group_enclosures
            .draw_under(painted_with(&painted_row, &()), renderer, theme);
        self.hover_glow.draw_under(
            painted_with(&painted_row, &painted_row.painter_state.slot_glows),
            renderer,
            theme,
        );
        self.travelling_highlight.draw_under(
            painted_with(&painted_row, &painted_row.painter_state.highlight_travel),
            renderer,
            theme,
        );
        self.alarm_tiles
            .draw_under(painted_with(&painted_row, &()), renderer, theme);
    }

    fn draw_over(
        &self,
        painted_row: PaintedRow<'_, RowPainterStates>,
        renderer: &mut iced::Renderer,
        theme: &Theme,
    ) {
        self.travelling_highlight.draw_over(
            painted_with(&painted_row, &painted_row.painter_state.highlight_travel),
            renderer,
            theme,
        );
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
