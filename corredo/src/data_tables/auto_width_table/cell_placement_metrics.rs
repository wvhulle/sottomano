use std::ops::Range;

use iced::advanced::layout::Node;

use super::{AutoWidthTable, RowSpan};
use crate::width_layout::fair_column_widths;

#[derive(Debug, Clone, Copy)]
pub(super) struct PlacedCell {
    pub(super) row: usize,
    pub(super) column: usize,
    pub(super) position: usize,
    pub(super) row_span: RowSpan,
}

impl<Message> AutoWidthTable<'_, Message> {
    pub(super) fn fitted_column_widths(
        &self,
        placed_cells: &[PlacedCell],
        nodes: &[Node],
        max_width: f32,
    ) -> Vec<f32> {
        let natural_widths: Vec<f32> = (0..self.column_count())
            .map(|column| {
                placed_cells
                    .iter()
                    .filter(|placed| {
                        placed.row_span == RowSpan::Columned && placed.column == column
                    })
                    .filter_map(|placed| nodes.get(placed.position))
                    .map(|node| node.size().width)
                    .fold(0.0, f32::max)
            })
            .collect();
        let room_for_cells = max_width - total_gap(natural_widths.len(), self.column_gap);
        if natural_widths.iter().sum::<f32>() > room_for_cells {
            fair_column_widths(&natural_widths, room_for_cells)
        } else {
            natural_widths
        }
    }

    fn column_count(&self) -> usize {
        self.alignments.len().max(1)
    }

    pub(super) fn placed_cells(&self) -> Vec<PlacedCell> {
        let column_count = self.column_count();
        self.row_spans
            .iter()
            .enumerate()
            .scan(0, |next_cell, (row, row_span)| {
                let cell_count = match row_span {
                    RowSpan::Columned => column_count,
                    RowSpan::FullWidth => 1,
                };
                let row_cells: Range<usize> = *next_cell..*next_cell + cell_count;
                *next_cell = row_cells.end;
                Some(
                    row_cells
                        .enumerate()
                        .map(move |(column, position)| PlacedCell {
                            row,
                            column,
                            position,
                            row_span: *row_span,
                        }),
                )
            })
            .flatten()
            .collect()
    }
}

#[allow(clippy::cast_precision_loss)]
fn total_gap(cell_count: usize, gap: f32) -> f32 {
    cell_count
        .checked_sub(1)
        .map_or(0.0, |gap_count| gap * gap_count as f32)
}

pub(super) fn span(extents: &[f32], gap: f32) -> f32 {
    extents.iter().sum::<f32>() + total_gap(extents.len(), gap)
}

pub(super) fn cell_starts(extents: &[f32], gap: f32) -> Vec<f32> {
    extents
        .iter()
        .scan(0.0, |start, extent| {
            let current = *start;
            *start += extent + gap;
            Some(current)
        })
        .collect()
}
