use iced::{Theme, advanced::Renderer as _};

use super::{
    UngroupedSlotLook,
    slot_underlay::{PaintedRow, SlotPainter},
};
use crate::orientation::Axis;

pub(super) struct UngroupedTiles<'view> {
    pub(super) pager_axis: Axis,
    pub(super) ungrouped_look: UngroupedSlotLook<'view>,
    pub(super) slot_positions: Vec<usize>,
}

impl SlotPainter for UngroupedTiles<'_> {
    type PainterState = ();

    fn draw_under(
        &self,
        painted_row: PaintedRow<'_, Self::PainterState>,
        renderer: &mut iced::Renderer,
        theme: &Theme,
    ) {
        let UngroupedSlotLook::RestingTile {
            tile_inset,
            resting_color,
        } = &self.ungrouped_look
        else {
            return;
        };
        let resting_fill = resting_color(theme);
        for slot_position in &self.slot_positions {
            if let Some(slot) = painted_row.child_bounds.get(*slot_position) {
                renderer.fill_quad(
                    tile_inset.tile_on_slot(self.pager_axis, *slot),
                    resting_fill,
                );
            }
        }
    }
}
