use core::slice;

use iced::{
    Element, Event, Length, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    border::Radius,
    mouse::Cursor,
    window,
};
use iced_exwlshell::{BlurRegions, InputRegionActions, IntoRegionAction, reexport::BlurRegion};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RegionRoles {
    pub takes_input: bool,
    pub blurred: bool,
    pub corner_radius: Radius,
}

impl RegionRoles {
    pub fn report(self, rects: &[Rectangle], bounds: Rectangle, operation: &mut dyn Operation) {
        operation.custom(None, bounds, &mut self.blur_rows(rects));
        operation.custom(None, bounds, &mut self.input_region_actions(rects));
    }

    fn input_region_actions(self, rects: &[Rectangle]) -> InputRegionActions {
        let input_rects = if self.takes_input { rects } else { &[] };
        InputRegionActions(
            input_rects
                .iter()
                .copied()
                .filter(|rect| rect.width > 0.0 && rect.height > 0.0)
                .chain([Rectangle::default()])
                .filter_map(|rect| rect.try_into_region_action().ok())
                .collect(),
        )
    }

    fn blur_rows(self, rects: &[Rectangle]) -> BlurRegions {
        if !self.blurred {
            return replacing_blur_regions(Vec::new());
        }
        replacing_blur_regions(
            rects
                .iter()
                .flat_map(|rect| rounded_blur_rows(*rect, self.corner_radius))
                .collect(),
        )
    }
}

pub(crate) fn report_input_across(surface: Rectangle, operation: &mut dyn Operation) {
    let whole_surface = surface.try_into_region_action().into_iter().collect();
    operation.custom(None, surface, &mut InputRegionActions(whole_surface));
}

pub(crate) fn replacing_blur_regions(rows: Vec<BlurRegion>) -> BlurRegions {
    BlurRegions(rows.into_iter().chain([BlurRegion::default()]).collect())
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn corner_inset(radius: i32, step: i32) -> i32 {
    if step >= radius {
        return 0;
    }
    let from_edge = (radius - step) as f32 - 0.5;
    let reach = (radius as f32)
        .mul_add(radius as f32, -from_edge * from_edge)
        .sqrt();
    (radius as f32 - reach).ceil() as i32
}

#[allow(clippy::cast_possible_truncation)]
pub(crate) fn rounded_blur_rows(rect: Rectangle, radius: Radius) -> Vec<BlurRegion> {
    let x = rect.x.ceil() as i32;
    let y = rect.y.ceil() as i32;
    let width = (rect.x + rect.width).floor() as i32 - x;
    let height = (rect.y + rect.height).floor() as i32 - y;
    if width <= 0 || height <= 0 {
        return Vec::new();
    }
    let fitted = |corner: f32| (corner.round() as i32).clamp(0, width.min(height) / 2);
    let top_left = fitted(radius.top_left);
    let top_right = fitted(radius.top_right);
    let bottom_left = fitted(radius.bottom_left);
    let bottom_right = fitted(radius.bottom_right);
    let row = |top: i32, left: i32, right: i32| BlurRegion {
        x: x + left,
        y: top,
        width: width - left - right,
        height: 1,
    };
    let top_rows = top_left.max(top_right);
    let bottom_rows = bottom_left.max(bottom_right);
    let top_corner_rows = (0..top_rows).map(|step| {
        row(
            y + step,
            corner_inset(top_left, step),
            corner_inset(top_right, step),
        )
    });
    let bottom_corner_rows = (0..bottom_rows).map(|step| {
        row(
            y + height - 1 - step,
            corner_inset(bottom_left, step),
            corner_inset(bottom_right, step),
        )
    });
    top_corner_rows
        .chain(bottom_corner_rows)
        .chain([BlurRegion {
            x,
            y: y + top_rows,
            width,
            height: height - top_rows - bottom_rows,
        }])
        .filter(|row| row.width > 0 && row.height > 0)
        .collect()
}

pub fn region_reporter<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    roles: RegionRoles,
) -> RegionReporter<'a, Message> {
    RegionReporter {
        content: content.into(),
        roles,
        on_width_change: None,
    }
}

pub struct RegionReporter<'a, Message> {
    content: Element<'a, Message>,
    roles: RegionRoles,
    on_width_change: Option<Box<dyn Fn(f32) -> Message + 'a>>,
}

impl<'a, Message> RegionReporter<'a, Message> {
    #[must_use]
    pub fn on_width_change(mut self, on_width_change: impl Fn(f32) -> Message + 'a) -> Self {
        self.on_width_change = Some(Box::new(on_width_change));
        self
    }
}

impl<'a, Message: 'a> From<RegionReporter<'a, Message>> for Element<'a, Message> {
    fn from(reporter: RegionReporter<'a, Message>) -> Self {
        Element::new(reporter)
    }
}

fn region_rects(layout: Layout<'_>) -> impl Iterator<Item = Rectangle> {
    layout
        .children()
        .next()
        .unwrap()
        .children()
        .map(|child| child.bounds())
}

#[derive(Default)]
struct LastWidth(Option<f32>);

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for RegionReporter<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<LastWidth>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(LastWidth::default())
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
        let rects: Vec<Rectangle> = region_rects(layout).collect();
        self.roles.report(&rects, layout.bounds(), operation);
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
        let Some(on_width_change) = &self.on_width_change else {
            return;
        };
        if !matches!(event, Event::Window(window::Event::RedrawRequested(_))) {
            return;
        }
        let width = region_rects(layout)
            .map(|rect| rect.width)
            .fold(0.0, f32::max);
        let last = tree.state.downcast_mut::<LastWidth>();
        if last.0 != Some(width) {
            last.0 = Some(width);
            shell.publish(on_width_change(width));
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
    use iced::{Rectangle, border::Radius};

    use super::{RegionRoles, rounded_blur_rows};

    fn widget_group_frame() -> Rectangle {
        Rectangle {
            x: 10.0,
            y: 0.0,
            width: 60.0,
            height: 24.0,
        }
    }

    #[test]
    fn roles_without_input_or_blur_still_replace_earlier_regions() {
        let roles = RegionRoles::default();
        let frames = [widget_group_frame()];
        assert!(!roles.input_region_actions(&frames).0.is_empty());
        assert!(!roles.blur_rows(&frames).0.is_empty());
    }

    #[test]
    fn taking_input_adds_collapsed_rects_only_as_the_empty_replacement() {
        let roles = RegionRoles {
            takes_input: true,
            ..RegionRoles::default()
        };
        let collapsed = Rectangle {
            width: 0.0,
            ..widget_group_frame()
        };
        let reported = roles.input_region_actions(&[widget_group_frame(), collapsed]);
        assert_eq!(reported.0.len(), 2);
    }

    #[test]
    fn a_frame_thinner_than_a_pixel_blurs_nothing() {
        let collapsed = Rectangle {
            x: 10.4,
            y: 0.0,
            width: 0.3,
            height: 24.0,
        };
        assert!(rounded_blur_rows(collapsed, Radius::from(8.0)).is_empty());
    }

    #[test]
    fn a_square_frame_blurs_as_one_rectangle() {
        let region = rounded_blur_rows(widget_group_frame(), Radius::from(0.0));
        assert_eq!(region.len(), 1);
        assert_eq!(
            (region[0].x, region[0].width, region[0].height),
            (10, 60, 24)
        );
    }

    #[test]
    fn rounded_corners_step_inwards_towards_the_edge() {
        let region = rounded_blur_rows(widget_group_frame(), Radius::from(8.0));
        let top_row = region.iter().find(|row| row.y == 0).expect("top row");
        let middle = region.iter().find(|row| row.height > 1).expect("middle");
        assert!(top_row.x > 10 && top_row.width < 60);
        assert_eq!((middle.y, middle.height, middle.width), (8, 8, 60));
    }

    #[test]
    fn the_radius_never_exceeds_half_the_frame() {
        let region = rounded_blur_rows(widget_group_frame(), Radius::from(100.0));
        assert!(region.iter().all(|row| row.width > 0 && row.height > 0));
    }

    #[test]
    fn square_top_corners_blur_the_full_top_row() {
        let bottom_only = Radius {
            top_left: 0.0,
            top_right: 0.0,
            bottom_right: 8.0,
            bottom_left: 8.0,
        };
        let region = rounded_blur_rows(widget_group_frame(), bottom_only);
        let top_row = region.iter().find(|row| row.y == 0).expect("top row");
        let bottom_row = region.iter().find(|row| row.y == 23).expect("bottom row");
        assert_eq!((top_row.x, top_row.width), (10, 60));
        assert!(bottom_row.x > 10 && bottom_row.width < 60);
    }

    #[test]
    fn each_side_steps_in_by_its_own_corner() {
        let left_only = Radius {
            top_left: 8.0,
            top_right: 0.0,
            bottom_right: 0.0,
            bottom_left: 8.0,
        };
        let region = rounded_blur_rows(widget_group_frame(), left_only);
        let top_row = region.iter().find(|row| row.y == 0).expect("top row");
        assert!(top_row.x > 10);
        assert_eq!(top_row.x + top_row.width, 70);
    }
}
