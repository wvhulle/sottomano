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

pub struct ContentSizedTable<'a, Message> {
    cells: Vec<Element<'a, Message>>,
    alignments: Vec<CellAlignment>,
    padding_x: f32,
    padding_y: f32,
}

pub fn content_sized_table<'a, Row: Clone, Message>(
    columns: impl IntoIterator<Item = ContentSizedColumn<'a, Row, Message>>,
    rows: impl IntoIterator<Item = Row>,
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
    let body_cells: Vec<Element<'a, Message>> = rows
        .into_iter()
        .flat_map(|row| {
            views
                .iter()
                .map(|(view, _)| view(row.clone()))
                .collect::<Vec<_>>()
        })
        .collect();
    ContentSizedTable {
        cells: headers.into_iter().chain(body_cells).collect(),
        alignments,
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
        let row_count = self.cells.len().div_ceil(column_count);
        let intrinsic = Limits::with_compression(Size::ZERO, limits.max(), Size::new(true, true));
        let mut nodes: Vec<Node> = self
            .cells
            .iter_mut()
            .zip(&mut tree.children)
            .map(|(cell, cell_tree)| cell.as_widget_mut().layout(cell_tree, renderer, &intrinsic))
            .collect();
        let column_widths: Vec<f32> = (0..column_count)
            .map(|column| {
                nodes
                    .iter()
                    .skip(column)
                    .step_by(column_count)
                    .map(|node| node.size().width)
                    .fold(0.0, f32::max)
            })
            .collect();
        self.cells
            .iter_mut()
            .zip(&mut tree.children)
            .zip(&mut nodes)
            .enumerate()
            .filter(|(_, ((cell, _), _))| cell.as_widget().size().width.is_fill())
            .for_each(|(position, ((cell, cell_tree), node))| {
                let column_width = column_widths[position % column_count];
                let column_limits = Limits::new(
                    Size::new(column_width, 0.0),
                    Size::new(column_width, limits.max().height),
                );
                *node = cell
                    .as_widget_mut()
                    .layout(cell_tree, renderer, &column_limits);
            });
        let row_heights: Vec<f32> = (0..row_count)
            .map(|row| {
                nodes
                    .iter()
                    .skip(row * column_count)
                    .take(column_count)
                    .map(|node| node.size().height)
                    .fold(0.0, f32::max)
            })
            .collect();
        let column_starts = cell_starts(&column_widths, self.padding_x);
        let row_starts = cell_starts(&row_heights, self.padding_y);
        for (position, node) in nodes.iter_mut().enumerate() {
            let column = position % column_count;
            let row = position / column_count;
            let alignment = self.alignments[column];
            node.move_to_mut((column_starts[column], row_starts[row]));
            node.align_mut(
                Alignment::from(alignment.align_x),
                Alignment::from(alignment.align_y),
                Size::new(column_widths[column], row_heights[row]),
            );
        }
        let span = |extents: &[f32], padding: f32| {
            extents.iter().sum::<f32>() + 2.0 * padding * extents.len() as f32
        };
        Node::with_children(
            Size::new(
                span(&column_widths, self.padding_x),
                span(&row_heights, self.padding_y),
            ),
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
