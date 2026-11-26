mod cell_placement_metrics;
pub(super) mod table_row_assembly;

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

use self::cell_placement_metrics::{cell_starts, span};

pub struct AutoWidthColumn<'view, Row, Message> {
    header: Element<'view, Message>,
    view: Box<dyn Fn(Row) -> Element<'view, Message> + 'view>,
    align_x: Horizontal,
    align_y: Vertical,
}

pub fn auto_width_column<'view, Row, Message, CellContent: Into<Element<'view, Message>>>(
    header: impl Into<Element<'view, Message>>,
    view: impl Fn(Row) -> CellContent + 'view,
) -> AutoWidthColumn<'view, Row, Message> {
    AutoWidthColumn {
        header: header.into(),
        view: Box::new(move |row| view(row).into()),
        align_x: Horizontal::Left,
        align_y: Vertical::Top,
    }
}

impl<Row, Message> AutoWidthColumn<'_, Row, Message> {
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

pub enum SpannableRow<'view, Row, Message> {
    Columned(Row),
    FullWidth(Element<'view, Message>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowSpan {
    Columned,
    FullWidth,
}

pub struct AutoWidthTable<'view, Message> {
    cells: Vec<Element<'view, Message>>,
    row_spans: Vec<RowSpan>,
    alignments: Vec<CellAlignment>,
    column_gap: f32,
    row_gap: f32,
}

impl<'view, Message: 'view> From<AutoWidthTable<'view, Message>> for Element<'view, Message> {
    fn from(table: AutoWidthTable<'view, Message>) -> Self {
        Element::new(table)
    }
}

impl<Message> AutoWidthTable<'_, Message> {
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
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for AutoWidthTable<'_, Message> {
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

    fn overlay<'tree>(
        &'tree mut self,
        tree: &'tree mut Tree,
        layout: Layout<'tree>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'tree, Message, iced::Theme, iced::Renderer>> {
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
