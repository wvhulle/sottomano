use std::ops::Range;

use iced::{
    Alignment, Element, Event, Length, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        overlay, renderer,
        widget::{Operation, Tree},
    },
    alignment::{Horizontal, Vertical},
    mouse::{Cursor, Interaction},
    widget::Space,
};

use crate::widgets::fair_widths::fair_widths;

pub struct ContentSizedColumn<'a, Row, Message> {
    header: Element<'a, Message>,
    view: Box<dyn Fn(Row) -> Element<'a, Message> + 'a>,
    align_x: Horizontal,
    align_y: Vertical,
}

pub fn content_sized_column<'a, Row, Message, CellContent: Into<Element<'a, Message>>>(
    header: impl Into<Element<'a, Message>>,
    view: impl Fn(Row) -> CellContent + 'a,
) -> ContentSizedColumn<'a, Row, Message> {
    ContentSizedColumn {
        header: header.into(),
        view: Box::new(move |row| view(row).into()),
        align_x: Horizontal::Left,
        align_y: Vertical::Top,
    }
}

impl<Row, Message> ContentSizedColumn<'_, Row, Message> {
    #[must_use]
    pub fn align_x(self, align_x: Horizontal) -> Self {
        Self { align_x, ..self }
    }

    #[must_use]
    pub fn align_y(self, align_y: Vertical) -> Self {
        Self { align_y, ..self }
    }
}

#[derive(Clone, Copy)]
struct CellAlignment {
    align_x: Horizontal,
    align_y: Vertical,
}

pub enum SpannableRow<'a, Row, Message> {
    Columned(Row),
    FullWidth(Element<'a, Message>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowSpan {
    Columned,
    FullWidth,
}

pub struct ContentSizedTable<'a, Message> {
    cells: Vec<Element<'a, Message>>,
    row_spans: Vec<RowSpan>,
    alignments: Vec<CellAlignment>,
    column_gap: f32,
    row_gap: f32,
}

pub fn content_sized_table<'a, Row: Clone, Message>(
    columns: impl IntoIterator<Item = ContentSizedColumn<'a, Row, Message>>,
    rows: impl IntoIterator<Item = Row>,
) -> ContentSizedTable<'a, Message> {
    content_sized_spanning_table(columns, rows.into_iter().map(SpannableRow::Columned))
}

pub fn content_sized_spanning_table<'a, Row: Clone, Message>(
    columns: impl IntoIterator<Item = ContentSizedColumn<'a, Row, Message>>,
    rows: impl IntoIterator<Item = SpannableRow<'a, Row, Message>>,
) -> ContentSizedTable<'a, Message> {
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
    ContentSizedTable {
        cells: table_cells.cells,
        row_spans: table_cells.row_spans,
        alignments,
        column_gap: 0.0,
        row_gap: 0.0,
    }
}

struct ColumnView<'a, Row, Message> {
    view: Box<dyn Fn(Row) -> Element<'a, Message> + 'a>,
    alignment: CellAlignment,
}

struct ColumnParts<'a, Row, Message> {
    headers: Vec<Element<'a, Message>>,
    views: Vec<ColumnView<'a, Row, Message>>,
}

impl<'a, Row, Message> ColumnParts<'a, Row, Message> {
    fn empty() -> Self {
        Self {
            headers: Vec::new(),
            views: Vec::new(),
        }
    }

    fn with_column(mut self, column: ContentSizedColumn<'a, Row, Message>) -> Self {
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

struct TableCells<'a, Message> {
    cells: Vec<Element<'a, Message>>,
    row_spans: Vec<RowSpan>,
}

impl<'a, Message> TableCells<'a, Message> {
    fn with_row<Row: Clone>(
        mut self,
        row: SpannableRow<'a, Row, Message>,
        views: &[ColumnView<'a, Row, Message>],
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

#[derive(Debug, Clone, Copy)]
struct PlacedCell {
    row: usize,
    column: usize,
    position: usize,
    row_span: RowSpan,
}

pub fn content_sized_grid<'a, Message: 'a>(
    column_alignments: Vec<Horizontal>,
    rows: impl IntoIterator<Item = Vec<Element<'a, Message>>>,
) -> ContentSizedTable<'a, Message> {
    content_sized_spanning_grid(
        column_alignments,
        rows.into_iter().map(SpannableRow::Columned),
    )
}

pub fn content_sized_spanning_grid<'a, Message: 'a>(
    column_alignments: Vec<Horizontal>,
    rows: impl IntoIterator<Item = SpannableRow<'a, Vec<Element<'a, Message>>, Message>>,
) -> ContentSizedTable<'a, Message> {
    let column_count = column_alignments.len();
    let (row_spans, cells): (Vec<RowSpan>, Vec<Vec<Element<'a, Message>>>) = rows
        .into_iter()
        .map(|row| match row {
            SpannableRow::Columned(mut row_cells) => {
                row_cells.resize_with(column_count, || Space::new().into());
                (RowSpan::Columned, row_cells)
            }
            SpannableRow::FullWidth(element) => (RowSpan::FullWidth, vec![element]),
        })
        .unzip();
    ContentSizedTable {
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

impl<'a, Message: 'a> From<ContentSizedTable<'a, Message>> for Element<'a, Message> {
    fn from(table: ContentSizedTable<'a, Message>) -> Self {
        Element::new(table)
    }
}

impl<Message> ContentSizedTable<'_, Message> {
    #[must_use]
    pub fn column_gap(self, column_gap: f32) -> Self {
        Self { column_gap, ..self }
    }

    #[must_use]
    pub fn row_gap(self, row_gap: f32) -> Self {
        Self { row_gap, ..self }
    }

    #[must_use]
    pub fn cells_aligned_y(mut self, align_y: Vertical) -> Self {
        self.alignments
            .iter_mut()
            .for_each(|alignment| alignment.align_y = align_y);
        self
    }

    fn fitted_column_widths(
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
            fair_widths(&natural_widths, room_for_cells)
        } else {
            natural_widths
        }
    }

    fn column_count(&self) -> usize {
        self.alignments.len().max(1)
    }

    fn placed_cells(&self) -> Vec<PlacedCell> {
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

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for ContentSizedTable<'_, Message> {
    fn children(&self) -> Vec<Tree> {
        self.cells.iter().map(Tree::new).collect()
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&self.cells);
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let row_count = self.row_spans.len();
        let placed_cells = self.placed_cells();
        let intrinsic = Limits::with_compression(Size::ZERO, limits.max(), Size::new(true, true));
        let mut nodes: Vec<Node> = self
            .cells
            .iter_mut()
            .zip(&mut tree.children)
            .map(|(cell, cell_tree)| cell.as_widget_mut().layout(cell_tree, renderer, &intrinsic))
            .collect();
        let column_widths = self.fitted_column_widths(&placed_cells, &nodes, limits.max().width);
        let table_width = span(&column_widths, self.column_gap);
        placed_cells.iter().for_each(|placed| {
            let cell = &mut self.cells[placed.position];
            let fixed_width = match placed.row_span {
                RowSpan::FullWidth => table_width,
                RowSpan::Columned if cell.as_widget().size().width.is_fill() => {
                    column_widths[placed.column]
                }
                RowSpan::Columned
                    if nodes[placed.position].size().width > column_widths[placed.column] =>
                {
                    let narrowed = Limits::new(
                        Size::ZERO,
                        Size::new(column_widths[placed.column], limits.max().height),
                    );
                    nodes[placed.position] = cell.as_widget_mut().layout(
                        &mut tree.children[placed.position],
                        renderer,
                        &narrowed,
                    );
                    return;
                }
                RowSpan::Columned => return,
            };
            let width_limits = Limits::new(
                Size::new(fixed_width, 0.0),
                Size::new(fixed_width, limits.max().height),
            );
            nodes[placed.position] = cell.as_widget_mut().layout(
                &mut tree.children[placed.position],
                renderer,
                &width_limits,
            );
        });
        let row_heights: Vec<f32> = (0..row_count)
            .map(|row| {
                placed_cells
                    .iter()
                    .filter(|placed| placed.row == row)
                    .map(|placed| nodes[placed.position].size().height)
                    .fold(0.0, f32::max)
            })
            .collect();
        let column_starts = cell_starts(&column_widths, self.column_gap);
        let row_starts = cell_starts(&row_heights, self.row_gap);
        placed_cells.iter().for_each(|placed| {
            let node = &mut nodes[placed.position];
            match placed.row_span {
                RowSpan::Columned => {
                    let alignment = self.alignments[placed.column];
                    node.move_to_mut((column_starts[placed.column], row_starts[placed.row]));
                    node.align_mut(
                        Alignment::from(alignment.align_x),
                        Alignment::from(alignment.align_y),
                        Size::new(column_widths[placed.column], row_heights[placed.row]),
                    );
                }
                RowSpan::FullWidth => {
                    node.move_to_mut((0.0, row_starts[placed.row]));
                    node.align_mut(
                        Alignment::Start,
                        Alignment::Center,
                        Size::new(table_width, row_heights[placed.row]),
                    );
                }
            }
        });
        Node::with_children(
            Size::new(table_width, span(&row_heights, self.row_gap)),
            nodes,
        )
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.cells
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
            .for_each(|((cell, cell_tree), cell_layout)| {
                cell.as_widget_mut().update(
                    cell_tree,
                    event,
                    cell_layout,
                    cursor,
                    renderer,
                    clipboard,
                    shell,
                    viewport,
                );
            });
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        self.cells
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .for_each(|((cell, cell_tree), cell_layout)| {
                cell.as_widget().draw(
                    cell_tree,
                    renderer,
                    theme,
                    style,
                    cell_layout,
                    cursor,
                    viewport,
                );
            });
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> Interaction {
        self.cells
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .map(|((cell, cell_tree), cell_layout)| {
                cell.as_widget().mouse_interaction(
                    cell_tree,
                    cell_layout,
                    cursor,
                    viewport,
                    renderer,
                )
            })
            .max()
            .unwrap_or_default()
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds());
        operation.traverse(&mut |operation| {
            self.cells
                .iter_mut()
                .zip(&mut tree.children)
                .zip(layout.children())
                .for_each(|((cell, cell_tree), cell_layout)| {
                    cell.as_widget_mut()
                        .operate(cell_tree, cell_layout, renderer, operation);
                });
        });
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        overlay::from_children(
            &mut self.cells,
            tree,
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

#[allow(clippy::cast_precision_loss)]
fn total_gap(cell_count: usize, gap: f32) -> f32 {
    gap * cell_count.saturating_sub(1) as f32
}

fn span(extents: &[f32], gap: f32) -> f32 {
    extents.iter().sum::<f32>() + total_gap(extents.len(), gap)
}

fn cell_starts(extents: &[f32], gap: f32) -> Vec<f32> {
    extents
        .iter()
        .scan(0.0, |start, extent| {
            let current = *start;
            *start += extent + gap;
            Some(current)
        })
        .collect()
}
