use core::slice;

use iced::{
    Element, Event, Length, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    mouse::Cursor,
    window,
};
use iced_exwlshell::{BlurRows, reexport::BlurRegion};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RegionRoles {
    pub takes_input: bool,
    pub blurred: bool,
    pub corner_radius: f32,
}

impl RegionRoles {
    pub fn blur_rows(self, rects: impl IntoIterator<Item = Rectangle>) -> BlurRows {
        if !self.blurred {
            return BlurRows::default();
        }
        BlurRows(
            rects
                .into_iter()
                .flat_map(|rect| rounded_blur_rows(rect, self.corner_radius))
                .collect(),
        )
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn rounded_blur_rows(rect: Rectangle, radius: f32) -> Vec<BlurRegion> {
    let x = rect.x.ceil() as i32;
    let y = rect.y.ceil() as i32;
    let width = (rect.x + rect.width).floor() as i32 - x;
    let height = (rect.y + rect.height).floor() as i32 - y;
    let radius = (radius.round() as i32).clamp(0, width.min(height) / 2);
    let row = |top: i32, inset: i32| BlurRegion {
        x: x + inset,
        y: top,
        width: width - 2 * inset,
        height: 1,
    };
    let corner_rows = (0..radius).flat_map(|step| {
        let from_edge = (radius - step) as f32 - 0.5;
        let reach = (radius as f32)
            .mul_add(radius as f32, -from_edge * from_edge)
            .sqrt();
        let inset = (radius as f32 - reach).ceil() as i32;
        [row(y + step, inset), row(y + height - 1 - step, inset)]
    });
    corner_rows
        .chain([BlurRegion {
            x,
            y: y + radius,
            width,
            height: height - 2 * radius,
        }])
        .filter(|row| row.width > 0 && row.height > 0)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReportedRegion {
    pub rects: Vec<Rectangle>,
    pub roles: RegionRoles,
}

pub fn region_reporter<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    roles: RegionRoles,
    on_report: impl Fn(ReportedRegion) -> Message + 'a,
) -> RegionReporter<'a, Message> {
    RegionReporter {
        content: content.into(),
        roles,
        on_report: Box::new(on_report),
    }
}

pub struct RegionReporter<'a, Message> {
    content: Element<'a, Message>,
    roles: RegionRoles,
    on_report: Box<dyn Fn(ReportedRegion) -> Message + 'a>,
}

impl<'a, Message: 'a> From<RegionReporter<'a, Message>> for Element<'a, Message> {
    fn from(reporter: RegionReporter<'a, Message>) -> Self {
        Element::new(reporter)
    }
}

#[derive(Default)]
struct LastReport(Option<ReportedRegion>);

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for RegionReporter<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<LastReport>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(LastReport::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let child = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        Node::with_children(child.size(), vec![child])
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
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout.children().next().unwrap(),
            cursor,
            viewport,
        );
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        let content_layout = layout.children().next().unwrap();
        let mut blur_rows = self
            .roles
            .blur_rows(content_layout.children().map(|child| child.bounds()));
        operation.custom(None, layout.bounds(), &mut blur_rows);
        self.content.as_widget_mut().operate(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            operation,
        );
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
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout.children().next().unwrap(),
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        if matches!(event, Event::Window(window::Event::RedrawRequested(_))) {
            let region = ReportedRegion {
                rects: layout
                    .children()
                    .next()
                    .unwrap()
                    .children()
                    .map(|child| child.bounds())
                    .collect(),
                roles: self.roles,
            };
            let last = tree.state.downcast_mut::<LastReport>();
            if last.0.as_ref() != Some(&region) {
                last.0 = Some(region.clone());
                shell.publish((self.on_report)(region));
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout.children().next().unwrap(),
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, iced::Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            viewport,
            translation,
        )
    }
}

#[cfg(test)]
mod tests {
    use iced::Rectangle;

    use super::rounded_blur_rows;

    fn widget_group_frame() -> Rectangle {
        Rectangle {
            x: 10.0,
            y: 0.0,
            width: 60.0,
            height: 24.0,
        }
    }

    #[test]
    fn a_square_frame_blurs_as_one_rectangle() {
        let region = rounded_blur_rows(widget_group_frame(), 0.0);
        assert_eq!(region.len(), 1);
        assert_eq!(
            (region[0].x, region[0].width, region[0].height),
            (10, 60, 24)
        );
    }

    #[test]
    fn rounded_corners_step_inwards_towards_the_edge() {
        let region = rounded_blur_rows(widget_group_frame(), 8.0);
        let top_row = region.iter().find(|row| row.y == 0).expect("top row");
        let middle = region.iter().find(|row| row.height > 1).expect("middle");
        assert!(top_row.x > 10 && top_row.width < 60);
        assert_eq!((middle.y, middle.height, middle.width), (8, 8, 60));
    }

    #[test]
    fn the_radius_never_exceeds_half_the_frame() {
        let region = rounded_blur_rows(widget_group_frame(), 100.0);
        assert!(region.iter().all(|row| row.width > 0 && row.height > 0));
    }
}
