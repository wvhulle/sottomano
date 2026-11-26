mod rounded_corner_rows;

use core::{iter, slice};

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

use self::rounded_corner_rows::rounded_blur_rows;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BackdropBlur {
    Blurred,
    #[default]
    Clear,
}

impl BackdropBlur {
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            BackdropBlur::Blurred => BackdropBlur::Clear,
            BackdropBlur::Clear => BackdropBlur::Blurred,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PointerInput {
    Accepted,
    #[default]
    PassedThrough,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RegionRoles {
    pub pointer_input: PointerInput,
    pub backdrop_blur: BackdropBlur,
    pub corner_radius: Radius,
}

impl RegionRoles {
    pub fn report(self, rects: &[Rectangle], bounds: Rectangle, operation: &mut dyn Operation) {
        self.report_with_blurred_subset(rects, rects, bounds, operation);
    }

    pub fn report_with_blurred_subset(
        self,
        rects: &[Rectangle],
        blurred_rects: &[Rectangle],
        bounds: Rectangle,
        operation: &mut dyn Operation,
    ) {
        operation.custom(None, bounds, &mut self.blur_rows(blurred_rects));
        operation.custom(None, bounds, &mut self.input_region_actions(rects));
    }

    fn input_region_actions(self, rects: &[Rectangle]) -> InputRegionActions {
        let input_rects = match self.pointer_input {
            PointerInput::Accepted => rects,
            PointerInput::PassedThrough => &[],
        };
        InputRegionActions(
            input_rects
                .iter()
                .copied()
                .filter(|rect| rect.width > 0.0 && rect.height > 0.0)
                .chain([Rectangle::default()])
                .map(|rect| {
                    rect.to_add()
                        .expect("rectangles with non-negative sides convert to region actions")
                })
                .collect(),
        )
    }

    pub(crate) fn blur_rows(self, rects: &[Rectangle]) -> BlurRegions {
        match self.backdrop_blur {
            BackdropBlur::Blurred => replacing_blur_regions(
                rects
                    .iter()
                    .flat_map(|rect| rounded_blur_rows(*rect, self.corner_radius))
                    .collect(),
            ),
            BackdropBlur::Clear => replacing_blur_regions(Vec::new()),
        }
    }
}

pub(crate) fn report_input_across_except(
    surface: Rectangle,
    holes: &[Rectangle],
    operation: &mut dyn Operation,
) {
    let surface_without_holes = iter::once(surface.to_add())
        .chain(holes.iter().copied().map(Rectangle::to_subtract))
        .filter_map(|region_action| {
            region_action
                .inspect_err(|cast_failure| {
                    tracing::debug!(
                        "input region: skipping rectangle of {surface:?}: {cast_failure}"
                    );
                })
                .ok()
        })
        .collect();
    operation.custom(
        None,
        surface,
        &mut InputRegionActions(surface_without_holes),
    );
}

pub(crate) fn replacing_blur_regions(rows: Vec<BlurRegion>) -> BlurRegions {
    BlurRegions(rows.into_iter().chain([BlurRegion::default()]).collect())
}

pub fn region_reporter<'view, Message: 'view>(
    content: impl Into<Element<'view, Message>>,
    roles: RegionRoles,
) -> RegionReporter<'view, Message> {
    RegionReporter {
        content: content.into(),
        roles,
        width_reporting: WidthReporting::Silent,
    }
}

pub struct RegionReporter<'view, Message> {
    content: Element<'view, Message>,
    roles: RegionRoles,
    width_reporting: WidthReporting<'view, Message>,
}

enum WidthReporting<'view, Message> {
    Silent,
    Reported(Box<dyn Fn(f32) -> Message + 'view>),
}

impl<'view, Message> RegionReporter<'view, Message> {
    #[must_use]
    pub fn on_width_change(mut self, on_width_change: impl Fn(f32) -> Message + 'view) -> Self {
        self.width_reporting = WidthReporting::Reported(Box::new(on_width_change));
        self
    }
}

impl<'view, Message: 'view> From<RegionReporter<'view, Message>> for Element<'view, Message> {
    fn from(reporter: RegionReporter<'view, Message>) -> Self {
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

#[derive(Default, PartialEq)]
enum LastWidth {
    #[default]
    Unmeasured,
    Measured(f32),
}

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
        let WidthReporting::Reported(on_width_change) = &self.width_reporting else {
            return;
        };
        if !matches!(event, Event::Window(window::Event::RedrawRequested(_))) {
            return;
        }
        let width = region_rects(layout)
            .map(|rect| rect.width)
            .fold(0.0, f32::max);
        let last_width = tree.state.downcast_mut::<LastWidth>();
        if *last_width != LastWidth::Measured(width) {
            *last_width = LastWidth::Measured(width);
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

    fn overlay<'tree>(
        &'tree mut self,
        tree: &'tree mut Tree,
        layout: Layout<'tree>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'tree, Message, iced::Theme, iced::Renderer>> {
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

    use super::{PointerInput, RegionRoles};

    fn indicator_group_frame() -> Rectangle {
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
        let frames = [indicator_group_frame()];
        assert!(!roles.input_region_actions(&frames).0.is_empty());
        assert!(!roles.blur_rows(&frames).0.is_empty());
    }

    #[test]
    fn taking_input_adds_collapsed_rects_only_as_the_empty_replacement() {
        let roles = RegionRoles {
            pointer_input: PointerInput::Accepted,
            ..RegionRoles::default()
        };
        let collapsed = Rectangle {
            width: 0.0,
            ..indicator_group_frame()
        };
        let reported = roles.input_region_actions(&[indicator_group_frame(), collapsed]);
        assert_eq!(reported.0.len(), 2);
    }
}
