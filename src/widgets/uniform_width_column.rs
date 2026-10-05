use iced::{
    Element, Event, Length, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        overlay, renderer,
        widget::{Operation, Tree},
    },
    mouse::{Cursor, Interaction},
};

pub fn uniform_width_column<'a, Message>(
    rows: impl IntoIterator<Item = impl Into<Element<'a, Message>>>,
) -> UniformWidthColumn<'a, Message> {
    UniformWidthColumn {
        rows: rows.into_iter().map(Into::into).collect(),
        spacing: 0.0,
    }
}

pub struct UniformWidthColumn<'a, Message> {
    rows: Vec<Element<'a, Message>>,
    spacing: f32,
}

impl<'a, Message: 'a> From<UniformWidthColumn<'a, Message>> for Element<'a, Message> {
    fn from(column: UniformWidthColumn<'a, Message>) -> Self {
        Element::new(column)
    }
}

impl<Message> UniformWidthColumn<'_, Message> {
    #[must_use]
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    fn widest_intrinsic_width(
        &mut self,
        trees: &mut [Tree],
        renderer: &iced::Renderer,
        limits: &Limits,
    ) -> f32 {
        let intrinsic = Limits::with_compression(
            Size::ZERO,
            limits.max(),
            Size::new(true, limits.compression().height),
        );
        self.rows
            .iter_mut()
            .zip(trees)
            .map(|(row, tree)| {
                row.as_widget_mut()
                    .layout(tree, renderer, &intrinsic)
                    .size()
                    .width
            })
            .fold(limits.min().width, f32::max)
    }
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for UniformWidthColumn<'_, Message> {
    fn children(&self) -> Vec<Tree> {
        self.rows.iter().map(Tree::new).collect()
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&self.rows);
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let width = self.widest_intrinsic_width(&mut tree.children, renderer, limits);
        let stretched = Limits::new(
            Size::new(width, 0.0),
            Size::new(width, limits.max().height),
        );
        let mut top = 0.0;
        let nodes = self
            .rows
            .iter_mut()
            .zip(&mut tree.children)
            .map(|(row, tree)| {
                let node = row
                    .as_widget_mut()
                    .layout(tree, renderer, &stretched)
                    .move_to((0.0, top));
                top += node.size().height + self.spacing;
                node
            })
            .collect();
        let height = (top - self.spacing).max(0.0);
        Node::with_children(Size::new(width, height), nodes)
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
        for ((row, tree), layout) in self
            .rows
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
        {
            row.as_widget_mut().update(
                tree, event, layout, cursor, renderer, clipboard, shell, viewport,
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
        for ((row, tree), layout) in self.rows.iter().zip(&tree.children).zip(layout.children()) {
            row.as_widget()
                .draw(tree, renderer, theme, style, layout, cursor, viewport);
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
        self.rows
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .map(|((row, tree), layout)| {
                row.as_widget()
                    .mouse_interaction(tree, layout, cursor, viewport, renderer)
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
            for ((row, tree), layout) in self
                .rows
                .iter_mut()
                .zip(&mut tree.children)
                .zip(layout.children())
            {
                row.as_widget_mut()
                    .operate(tree, layout, renderer, operation);
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
        overlay::from_children(&mut self.rows, tree, layout, renderer, viewport, translation)
    }
}
