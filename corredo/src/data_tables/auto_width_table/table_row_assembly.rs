use iced::{
    Element,
    alignment::{Horizontal, Vertical},
    widget::Space,
};

use super::{AutoWidthColumn, AutoWidthTable, CellAlignment, RowSpan, SpannableRow};

pub fn auto_width_table<'view, Row: Clone, Message>(
    columns: impl IntoIterator<Item = AutoWidthColumn<'view, Row, Message>>,
    rows: impl IntoIterator<Item = Row>,
) -> AutoWidthTable<'view, Message> {
    auto_width_spanning_table(columns, rows.into_iter().map(SpannableRow::Columned))
}

pub fn auto_width_spanning_table<'view, Row: Clone, Message>(
    columns: impl IntoIterator<Item = AutoWidthColumn<'view, Row, Message>>,
    rows: impl IntoIterator<Item = SpannableRow<'view, Row, Message>>,
) -> AutoWidthTable<'view, Message> {
    let column_parts = columns
        .into_iter()
        .fold(ColumnParts::empty(), ColumnParts::with_column);
    let alignments = column_parts
        .views
        .iter()
        .map(|column_view| column_view.alignment)
        .collect();
    let table_cells = rows.into_iter().fold(
        TableCells {
            cells: column_parts.headers,
            row_spans: vec![RowSpan::Columned],
        },
        |table_cells, row| table_cells.with_row(row, &column_parts.views),
    );
    AutoWidthTable {
        cells: table_cells.cells,
        row_spans: table_cells.row_spans,
        alignments,
        column_gap: 0.0,
        row_gap: 0.0,
    }
}

struct ColumnView<'view, Row, Message> {
    view: Box<dyn Fn(Row) -> Element<'view, Message> + 'view>,
    alignment: CellAlignment,
}

struct ColumnParts<'view, Row, Message> {
    headers: Vec<Element<'view, Message>>,
    views: Vec<ColumnView<'view, Row, Message>>,
}

impl<'view, Row, Message> ColumnParts<'view, Row, Message> {
    fn empty() -> Self {
        Self {
            headers: Vec::new(),
            views: Vec::new(),
        }
    }

    fn with_column(mut self, column: AutoWidthColumn<'view, Row, Message>) -> Self {
        self.headers.push(column.header);
        self.views.push(ColumnView {
            view: column.view,
            alignment: CellAlignment {
                align_x: column.align_x,
                align_y: column.align_y,
            },
        });
        self
    }
}

struct TableCells<'view, Message> {
    cells: Vec<Element<'view, Message>>,
    row_spans: Vec<RowSpan>,
}

impl<'view, Message> TableCells<'view, Message> {
    fn with_row<Row: Clone>(
        mut self,
        row: SpannableRow<'view, Row, Message>,
        views: &[ColumnView<'view, Row, Message>],
    ) -> Self {
        match row {
            SpannableRow::Columned(row) => {
                self.cells.extend(
                    views
                        .iter()
                        .map(|column_view| (column_view.view)(row.clone())),
                );
                self.row_spans.push(RowSpan::Columned);
            }
            SpannableRow::FullWidth(element) => {
                self.cells.push(element);
                self.row_spans.push(RowSpan::FullWidth);
            }
        }
        self
    }
}

pub fn auto_width_grid<'view, Message: 'view>(
    column_alignments: Vec<Horizontal>,
    rows: impl IntoIterator<Item = Vec<Element<'view, Message>>>,
) -> AutoWidthTable<'view, Message> {
    auto_width_spanning_grid(
        column_alignments,
        rows.into_iter().map(SpannableRow::Columned),
    )
}

pub fn auto_width_spanning_grid<'view, Message: 'view>(
    column_alignments: Vec<Horizontal>,
    rows: impl IntoIterator<Item = SpannableRow<'view, Vec<Element<'view, Message>>, Message>>,
) -> AutoWidthTable<'view, Message> {
    let column_count = column_alignments.len();
    let (row_spans, cells): (Vec<RowSpan>, Vec<Vec<Element<'view, Message>>>) = rows
        .into_iter()
        .map(|row| match row {
            SpannableRow::Columned(mut row_cells) => {
                row_cells.resize_with(column_count, || Space::new().into());
                (RowSpan::Columned, row_cells)
            }
            SpannableRow::FullWidth(element) => (RowSpan::FullWidth, vec![element]),
        })
        .unzip();
    AutoWidthTable {
        row_spans,
        cells: cells.into_iter().flatten().collect(),
        alignments: column_alignments
            .into_iter()
            .map(|align_x| CellAlignment {
                align_x,
                align_y: Vertical::Center,
            })
            .collect(),
        column_gap: 0.0,
        row_gap: 0.0,
    }
}
