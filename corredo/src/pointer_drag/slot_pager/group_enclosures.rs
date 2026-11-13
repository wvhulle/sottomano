use std::ops::Range;

use iced::{
    Rectangle, Theme,
    advanced::{Renderer as _, renderer},
    border,
};

use super::slot_underlay::{PaintedRow, SlotPainter};
use crate::{
    pointer_drag::slot_group::{PagerAxis, SlotTileInset, tile_quad},
    theme_colors::ThemedColor,
};

pub(super) struct EnclosedGroup<'a> {
    pub(super) heading_index: usize,
    pub(super) slot_positions: Range<usize>,
    pub(super) tile_inset: SlotTileInset,
    pub(super) enclosure_color: ThemedColor<'a>,
    pub(super) inner_color: ThemedColor<'a>,
}

pub(super) struct GroupEnclosures<'a> {
    pub(super) pager_axis: PagerAxis,
    pub(super) enclosed_groups: Vec<EnclosedGroup<'a>>,
}

fn enclosure_around(
    pager_axis: PagerAxis,
    heading: Rectangle,
    slot_cells: &[Rectangle],
    thickness: f32,
) -> Option<Rectangle> {
    let around_slot_cells = slot_cells
        .iter()
        .copied()
        .reduce(|covered, slot_cell| covered.union(&slot_cell))?
        .expand(thickness);
    let heading_along_row = match pager_axis {
        PagerAxis::Vertical => Rectangle {
            y: heading.y,
            height: heading.height,
            ..around_slot_cells
        },
        PagerAxis::Horizontal => Rectangle {
            x: heading.x,
            width: heading.width,
            ..around_slot_cells
        },
    };
    Some(around_slot_cells.union(&heading_along_row))
}

impl GroupEnclosures<'_> {
    fn slot_cells(
        &self,
        enclosed_group: &EnclosedGroup<'_>,
        child_bounds: &[Rectangle],
    ) -> Vec<Rectangle> {
        enclosed_group
            .slot_positions
            .clone()
            .filter_map(|slot_position| child_bounds.get(slot_position))
            .map(|slot| {
                enclosed_group
                    .tile_inset
                    .inner_surface(self.pager_axis, *slot)
            })
            .collect()
    }
}

impl SlotPainter for GroupEnclosures<'_> {
    type PainterState = ();

    fn draw_under(
        &self,
        painted_row: PaintedRow<'_, Self::PainterState>,
        renderer: &mut iced::Renderer,
        theme: &Theme,
    ) {
        for enclosed_group in &self.enclosed_groups {
            let Some(heading) = painted_row.child_bounds.get(enclosed_group.heading_index) else {
                continue;
            };
            let slot_cells = self.slot_cells(enclosed_group, painted_row.child_bounds);
            let Some(enclosure) = enclosure_around(
                self.pager_axis,
                *heading,
                &slot_cells,
                enclosed_group.tile_inset.thickness,
            ) else {
                continue;
            };
            renderer.fill_quad(
                renderer::Quad {
                    bounds: enclosure,
                    border: border::rounded(enclosed_group.tile_inset.enclosure_radius),
                    snap: true,
                    ..renderer::Quad::default()
                },
                (enclosed_group.enclosure_color)(theme),
            );
            let inner_color = (enclosed_group.inner_color)(theme);
            for slot_cell in slot_cells {
                renderer.fill_quad(
                    tile_quad(slot_cell, enclosed_group.tile_inset.inner_radius().into()),
                    inner_color,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use iced::Rectangle;

    use super::enclosure_around;
    use crate::pointer_drag::slot_group::{PagerAxis, SlotTileInset};

    const TILE_INSET: SlotTileInset = SlotTileInset {
        thickness: 3.0,
        enclosure_radius: 8.0,
    };

    #[test]
    fn the_enclosure_spans_its_heading_and_every_slot_cell() {
        let heading = Rectangle::new((0.0, 5.0).into(), (30.0, 20.0).into());
        let first_slot = Rectangle::new((33.0, 0.0).into(), (40.0, 30.0).into());
        let second_slot = Rectangle::new((76.0, 0.0).into(), (40.0, 30.0).into());
        let slot_cells = [first_slot, second_slot]
            .map(|slot| TILE_INSET.inner_surface(PagerAxis::Horizontal, slot));
        let enclosure = enclosure_around(PagerAxis::Horizontal, heading, &slot_cells, 3.0)
            .expect("two cells enclose a group");
        assert!((enclosure.x - heading.x).abs() < f32::EPSILON);
        assert!(
            slot_cells
                .iter()
                .all(|slot_cell| enclosure.contains(slot_cell.center()))
        );
        assert!((enclosure.height - first_slot.height).abs() < f32::EPSILON);
    }
}
