use iced::{Theme, advanced::Renderer as _};

use super::slot_underlay::{PaintedRow, SlotPainter};
use crate::{
    drag_reorder::slot_group::{SlotTileInset, TileNeighbours},
    orientation::Axis,
    theming::ThemedColor,
};

pub(super) struct AlarmTile<'a> {
    pub(super) slot_position: usize,
    pub(super) tile_inset: SlotTileInset,
    pub(super) tile_neighbours: TileNeighbours,
    pub(super) color: ThemedColor<'a>,
}

pub(super) struct AlarmTiles<'a> {
    pub(super) pager_axis: Axis,
    pub(super) alarm_tiles: Vec<AlarmTile<'a>>,
}

impl SlotPainter for AlarmTiles<'_> {
    type PainterState = ();

    fn draw_under(
        &self,
        painted_row: PaintedRow<'_, Self::PainterState>,
        renderer: &mut iced::Renderer,
        theme: &Theme,
    ) {
        for alarm_tile in &self.alarm_tiles {
            if let Some(slot) = painted_row.child_bounds.get(alarm_tile.slot_position) {
                renderer.fill_quad(
                    alarm_tile.tile_inset.tile_on_slot(
                        self.pager_axis,
                        *slot,
                        alarm_tile.tile_neighbours,
                    ),
                    (alarm_tile.color)(theme),
                );
            }
        }
    }
}
