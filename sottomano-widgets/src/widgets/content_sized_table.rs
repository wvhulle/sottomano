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
    padding_x: f32,
    padding_y: f32,
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
    let (headers, views): (Vec<_>, Vec<_>) = columns
        .into_iter()
        .map(|column| {
            (
                column.header,
                (
                    column.view,
                    CellAlignment {
                        align_x: column.align_x,
                        align_y: column.align_y,
                    },
                ),
            )
        })
        .unzip();
    let alignments = views.iter().map(|(_, alignment)| *alignment).collect();
    let mut cells = headers;
    let mut row_spans = vec![RowSpan::Columned];
    for row in rows {
        match row {
            SpannableRow::Columned(row) => {
                cells.extend(views.iter().map(|(view, _)| view(row.clone())));
                row_spans.push(RowSpan::Columned);
            }
            SpannableRow::FullWidth(element) => {
                cells.push(element);
                row_spans.push(RowSpan::FullWidth);
            }
        }
    }
    ContentSizedTable {
        cells,
        row_spans,
        alignments,
        padding_x: 0.0,
        padding_y: 0.0,
    }
}

pub fn content_sized_grid<'a, Message: 'a>(
    column_alignments: Vec<Horizontal>,
    rows: impl IntoIterator<Item = Vec<Element<'a, Message>>>,
) -> ContentSizedTable<'a, Message> {
    let column_count = column_alignments.len();
    let row_cells: Vec<Vec<Element<'a, Message>>> = rows
        .into_iter()
        .map(|mut row| {
            row.resize_with(column_count, || Space::new().into());
            row
        })
        .collect();
    ContentSizedTable {
        row_spans: vec![RowSpan::Columned; row_cells.len()],
        cells: row_cells.into_iter().flatten().collect(),
        alignments: column_alignments
            .into_iter()
            .map(|align_x| CellAlignment {
                align_x,
                align_y: Vertical::Center,
            })
            .collect(),
        padding_x: 0.0,
        padding_y: 0.0,
    }
}

impl<'a, Message: 'a> From<ContentSizedTable<'a, Message>> for Element<'a, Message> {
    fn from(table: ContentSizedTable<'a, Message>) -> Self {
        Element::new(table)
    }
}

impl<Message> ContentSizedTable<'_, Message> {
    #[must_use]
    pub fn padding_x(self, padding_x: f32) -> Self {
        Self { padding_x, ..self }
    }

    #[must_use]
    pub fn padding_y(self, padding_y: f32) -> Self {
        Self { padding_y, ..self }
    }

    fn column_count(&self) -> usize {
        self.alignments.len().max(1)
    }

    fn row_cell_ranges(&self) -> Vec<Range<usize>> {
        let column_count = self.column_count();
        self.row_spans
            .iter()
            .scan(0, |next_cell, row_span| {
                let cell_count = match row_span {
                    RowSpan::Columned => column_count,
                    RowSpan::FullWidth => 1,
                };
                let row_cells = *next_cell..*next_cell + cell_count;
                *next_cell = row_cells.end;
                Some(row_cells)
            })
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
        let column_count = self.column_count();
        let row_cells = self.row_cell_ranges();
        let row_spans = self.row_spans.clone();
        let intrinsic = Limits::with_compression(Size::ZERO, limits.max(), Size::new(true, true));
        let mut nodes: Vec<Node> = self
            .cells
            .iter_mut()
            .zip(&mut tree.children)
            .map(|(cell, cell_tree)| cell.as_widget_mut().layout(cell_tree, renderer, &intrinsic))
            .collect();
        let columned_row_starts: Vec<usize> = row_cells
            .iter()
            .zip(&row_spans)
            .filter(|(_, row_span)| **row_span == RowSpan::Columned)
            .map(|(cells, _)| cells.start)
            .collect();
        let column_widths: Vec<f32> = (0..column_count)
            .map(|column| {
                columned_row_starts
                    .iter()
                    .filter_map(|row_start| nodes.get(row_start + column))
                    .map(|node| node.size().width)
                    .fold(0.0, f32::max)
            })
            .collect();
        let span = |extents: &[f32], padding: f32| {
            extents.iter().sum::<f32>() + 2.0 * padding * extents.len() as f32
        };
        let table_width = span(&column_widths, self.padding_x);
        for (cells, row_span) in row_cells.iter().zip(&row_spans) {
            for (column, position) in cells.clone().enumerate() {
                let cell = &mut self.cells[position];
                let fixed_width = match row_span {
                    RowSpan::FullWidth => table_width,
                    RowSpan::Columned if cell.as_widget().size().width.is_fill() => {
                        column_widths[column]
                    }
                    RowSpan::Columned => continue,
                };
                let width_limits = Limits::new(
                    Size::new(fixed_width, 0.0),
                    Size::new(fixed_width, limits.max().height),
                );
                nodes[position] = cell.as_widget_mut().layout(
                    &mut tree.children[position],
                    renderer,
                    &width_limits,
                );
            }
        }
        let row_heights: Vec<f32> = row_cells
            .iter()
            .map(|cells| {
                nodes[cells.clone()]
                    .iter()
                    .map(|node| node.size().height)
                    .fold(0.0, f32::max)
            })
            .collect();
        let column_starts = cell_starts(&column_widths, self.padding_x);
        let row_starts = cell_starts(&row_heights, self.padding_y);
        for (row, (cells, row_span)) in row_cells.iter().zip(&row_spans).enumerate() {
            for (column, position) in cells.clone().enumerate() {
                let node = &mut nodes[position];
                match row_span {
                    RowSpan::Columned => {
                        let alignment = self.alignments[column];
                        node.move_to_mut((column_starts[column], row_starts[row]));
                        node.align_mut(
                            Alignment::from(alignment.align_x),
                            Alignment::from(alignment.align_y),
                            Size::new(column_widths[column], row_heights[row]),
                        );
                    }
                    RowSpan::FullWidth => {
                        node.move_to_mut((0.0, row_starts[row]));
                        node.align_mut(
                            Alignment::Start,
                            Alignment::Center,
                            Size::new(table_width, row_heights[row]),
                        );
                    }
                }
            }
        }
        Node::with_children(
            Size::new(table_width, span(&row_heights, self.padding_y)),
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
        for ((cell, cell_tree), cell_layout) in self
            .cells
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
        {
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
        }
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
        for ((cell, cell_tree), cell_layout) in
            self.cells.iter().zip(&tree.children).zip(layout.children())
        {
            cell.as_widget().draw(
                cell_tree,
                renderer,
                theme,
                style,
                cell_layout,
                cursor,
                viewport,
            );
        }
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
            for ((cell, cell_tree), cell_layout) in self
                .cells
                .iter_mut()
                .zip(&mut tree.children)
                .zip(layout.children())
            {
                cell.as_widget_mut()
                    .operate(cell_tree, cell_layout, renderer, operation);
            }
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

fn cell_starts(extents: &[f32], padding: f32) -> Vec<f32> {
    extents
        .iter()
        .scan(padding, |start, extent| {
            let current = *start;
            *start += extent + 2.0 * padding;
            Some(current)
        })
        .collect()
}
