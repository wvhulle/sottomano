use std::{collections::HashMap, iter, mem, slice};

use iced::{
    Color, Element, Event, Length, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    mouse::Cursor,
    window,
};

use crate::{theming::FrameStyle, transitions::uniform_alpha_fade::fade_toward_color};

pub struct SwapPane<'view, Message> {
    pub identity: u64,
    pub slide_order: u8,
    pub content: Element<'view, Message>,
}

pub enum DepartingPane<'view, Message> {
    Absent,
    Departing {
        pane: SwapPane<'view, Message>,
        slide_progress: f32,
    },
}

impl<'view, Message> DepartingPane<'view, Message> {
    fn as_slice(&self) -> &[SwapPane<'view, Message>] {
        match self {
            Self::Departing { pane, .. } => slice::from_ref(pane),
            Self::Absent => &[],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SlideDirection {
    EnterFromBelow,
    EnterFromAbove,
}

impl SlideDirection {
    fn between(arriving_order: u8, departing_order: u8) -> Self {
        if arriving_order >= departing_order {
            Self::EnterFromBelow
        } else {
            Self::EnterFromAbove
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SlideBand {
    OwnBounds,
    AlongTopEdge {
        frame_depth_px: f32,
        float_offset_px: f32,
        window_gap_px: f32,
    },
    AlongBottomEdge {
        frame_depth_px: f32,
        float_offset_px: f32,
        window_gap_px: f32,
    },
}

struct BandRegions {
    whole_band: Rectangle,
    framed_part: Rectangle,
    floating_gap: Rectangle,
    window_gap: Rectangle,
}

impl SlideBand {
    fn regions(self, bounds: Rectangle, viewport: &Rectangle) -> BandRegions {
        let horizontal_strip = |top: f32, height: f32| Rectangle {
            x: bounds.x,
            width: bounds.width,
            y: top,
            height,
        };
        match self {
            Self::OwnBounds => BandRegions {
                whole_band: bounds,
                framed_part: bounds,
                floating_gap: horizontal_strip(bounds.y, 0.0),
                window_gap: horizontal_strip(bounds.y, 0.0),
            },
            Self::AlongTopEdge {
                frame_depth_px,
                float_offset_px,
                window_gap_px,
            } => {
                let band_top = viewport.y;
                let frame_top = band_top + float_offset_px;
                BandRegions {
                    whole_band: horizontal_strip(
                        band_top,
                        float_offset_px + frame_depth_px + window_gap_px,
                    ),
                    framed_part: horizontal_strip(frame_top, frame_depth_px),
                    floating_gap: horizontal_strip(band_top, float_offset_px),
                    window_gap: horizontal_strip(frame_top + frame_depth_px, window_gap_px),
                }
            }
            Self::AlongBottomEdge {
                frame_depth_px,
                float_offset_px,
                window_gap_px,
            } => {
                let frame_top = viewport.y + viewport.height - float_offset_px - frame_depth_px;
                let band_top = frame_top - window_gap_px;
                BandRegions {
                    whole_band: horizontal_strip(
                        band_top,
                        window_gap_px + frame_depth_px + float_offset_px,
                    ),
                    framed_part: horizontal_strip(frame_top, frame_depth_px),
                    floating_gap: horizontal_strip(frame_top + frame_depth_px, float_offset_px),
                    window_gap: horizontal_strip(band_top, window_gap_px),
                }
            }
        }
    }
}

struct SlideShifts {
    departing_shift: f32,
    arriving_shift: f32,
}

impl SlideDirection {
    fn shifts(
        self,
        band: Rectangle,
        departing_bounds: Rectangle,
        arriving_bounds: Rectangle,
        progress: f32,
    ) -> SlideShifts {
        let band_bottom = band.y + band.height;
        let (leaving_distance, entering_distance) = match self {
            Self::EnterFromBelow => (
                departing_bounds.y + departing_bounds.height - band.y,
                band_bottom - arriving_bounds.y,
            ),
            Self::EnterFromAbove => (
                band_bottom - departing_bounds.y,
                arriving_bounds.y + arriving_bounds.height - band.y,
            ),
        };
        let shared_travel = leaving_distance.max(entering_distance);
        let upward_travel = match self {
            Self::EnterFromBelow => -shared_travel,
            Self::EnterFromAbove => shared_travel,
        };
        SlideShifts {
            departing_shift: upward_travel * progress,
            arriving_shift: -upward_travel * (1.0 - progress),
        }
    }
}

struct FadeTarget {
    region: Rectangle,
    restored_fill: Color,
}

pub struct SlidingPaneSwap<'view, Message> {
    arriving: SwapPane<'view, Message>,
    departing: DepartingPane<'view, Message>,
    fade_frame: FrameStyle,
    slide_band: SlideBand,
}

impl<Message> SlidingPaneSwap<'_, Message> {
    #[must_use]
    pub fn faded_into(self, fade_frame: FrameStyle) -> Self {
        Self { fade_frame, ..self }
    }

    #[must_use]
    pub fn sliding_within(self, slide_band: SlideBand) -> Self {
        Self { slide_band, ..self }
    }
}

#[must_use]
pub fn sliding_pane_swap<'view, Message: 'view>(
    arriving: SwapPane<'view, Message>,
    departing: DepartingPane<'view, Message>,
) -> SlidingPaneSwap<'view, Message> {
    SlidingPaneSwap {
        arriving,
        departing,
        fade_frame: FrameStyle::default(),
        slide_band: SlideBand::OwnBounds,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct BandMargins {
    above_px: f32,
    below_px: f32,
}

struct SwapState {
    band_margins: BandMargins,
    child_identities: Vec<u64>,
}

impl<'view, Message> SlidingPaneSwap<'view, Message> {
    fn panes(&self) -> impl Iterator<Item = &SwapPane<'view, Message>> {
        iter::once(&self.arriving).chain(self.departing.as_slice())
    }
}

fn centred_at(node: Node, height: f32) -> Node {
    let top = (height - node.size().height) / 2.0;
    node.move_to((0.0, top))
}

fn sliding_layout(
    arriving_node: Node,
    departing_node: Node,
    height: f32,
    band_margins: BandMargins,
    direction: SlideDirection,
    slide_progress: f32,
) -> Node {
    let arriving_width = arriving_node.size().width;
    let width = if arriving_width > 0.0 {
        arriving_width
    } else {
        departing_node.size().width
    };
    let band_in_own_coordinates = Rectangle {
        x: 0.0,
        y: -band_margins.above_px,
        width,
        height: height + band_margins.above_px + band_margins.below_px,
    };
    let SlideShifts {
        departing_shift,
        arriving_shift,
    } = direction.shifts(
        band_in_own_coordinates,
        departing_node.bounds(),
        arriving_node.bounds(),
        slide_progress,
    );
    Node::with_children(
        Size::new(width, height),
        vec![
            arriving_node.translate(Vector::new(0.0, arriving_shift)),
            departing_node.translate(Vector::new(0.0, departing_shift)),
        ],
    )
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for SlidingPaneSwap<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<SwapState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(SwapState {
            band_margins: BandMargins::default(),
            child_identities: self.panes().map(|pane| pane.identity).collect(),
        })
    }

    fn children(&self) -> Vec<Tree> {
        self.panes().map(|pane| Tree::new(&pane.content)).collect()
    }

    fn diff(&self, tree: &mut Tree) {
        let Tree {
            state, children, ..
        } = tree;
        let swap_state = state.downcast_mut::<SwapState>();
        let mut previous_trees: HashMap<u64, Tree> = mem::take(&mut swap_state.child_identities)
            .into_iter()
            .zip(mem::take(children))
            .collect();
        *children = self
            .panes()
            .map(|pane| match previous_trees.remove(&pane.identity) {
                Some(mut kept_tree) => {
                    kept_tree.diff(pane.content.as_widget());
                    kept_tree
                }
                None => Tree::new(&pane.content),
            })
            .collect();
        swap_state.child_identities = self.panes().map(|pane| pane.identity).collect();
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let band_margins = tree.state.downcast_ref::<SwapState>().band_margins;
        let arriving_node =
            self.arriving
                .content
                .as_widget_mut()
                .layout(&mut tree.children[0], renderer, limits);
        let DepartingPane::Departing {
            pane: departing,
            slide_progress,
        } = &mut self.departing
        else {
            let height = arriving_node.size().height;
            return Node::with_children(
                arriving_node.size(),
                vec![centred_at(arriving_node, height)],
            );
        };
        let departing_node =
            departing
                .content
                .as_widget_mut()
                .layout(&mut tree.children[1], renderer, limits);
        let height = arriving_node
            .size()
            .height
            .max(departing_node.size().height);
        sliding_layout(
            centred_at(arriving_node, height),
            centred_at(departing_node, height),
            height,
            band_margins,
            SlideDirection::between(self.arriving.slide_order, departing.slide_order),
            *slide_progress,
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
        if let Event::Window(window::Event::RedrawRequested(_)) = event {
            let swap_state = tree.state.downcast_mut::<SwapState>();
            let bounds = layout.bounds();
            let band = self.slide_band.regions(bounds, viewport).whole_band;
            let measured_margins = BandMargins {
                above_px: (bounds.y - band.y).max(0.0),
                below_px: (band.y + band.height - bounds.y - bounds.height).max(0.0),
            };
            if swap_state.band_margins != measured_margins {
                swap_state.band_margins = measured_margins;
                shell.invalidate_layout();
            }
        }
        self.arriving.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout.children().next().unwrap(),
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
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
        let mut pane_layouts = layout.children();
        let arriving_layout = pane_layouts.next().unwrap();
        let (
            DepartingPane::Departing {
                pane: departing,
                slide_progress,
            },
            Some(departing_layout),
        ) = (&self.departing, pane_layouts.next())
        else {
            self.arriving.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                arriving_layout,
                cursor,
                viewport,
            );
            return;
        };
        let regions = self.slide_band.regions(layout.bounds(), viewport);
        let Some(band_clip) = regions.whole_band.intersection(viewport) else {
            return;
        };
        let draw_pane = |renderer: &mut iced::Renderer,
                         content: &Element<'_, Message>,
                         pane_tree: &Tree,
                         pane_layout: Layout<'_>| {
            renderer.with_layer(band_clip, |renderer| {
                content.as_widget().draw(
                    pane_tree,
                    renderer,
                    theme,
                    style,
                    pane_layout,
                    Cursor::Unavailable,
                    &band_clip,
                );
            });
        };
        draw_pane(
            renderer,
            &departing.content,
            &tree.children[1],
            departing_layout,
        );
        [
            FadeTarget {
                region: regions.framed_part,
                restored_fill: self.fade_frame.fill_color(theme),
            },
            FadeTarget {
                region: regions.floating_gap,
                restored_fill: Color::TRANSPARENT,
            },
            FadeTarget {
                region: regions.window_gap,
                restored_fill: Color::TRANSPARENT,
            },
        ]
        .into_iter()
        .for_each(|fade_target| {
            let Some(faded_area) = departing_layout
                .bounds()
                .intersection(&fade_target.region)
                .and_then(|departing_area| departing_area.intersection(&band_clip))
            else {
                return;
            };
            renderer.with_layer(faded_area, |renderer| {
                fade_toward_color(
                    fade_target.restored_fill,
                    *slide_progress,
                    renderer,
                    faded_area,
                );
            });
        });
        draw_pane(
            renderer,
            &self.arriving.content,
            &tree.children[0],
            arriving_layout,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.arriving.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout.children().next().unwrap(),
            cursor,
            viewport,
            renderer,
        )
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.arriving.content.as_widget_mut().operate(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            operation,
        );
    }

    fn overlay<'tree>(
        &'tree mut self,
        tree: &'tree mut Tree,
        layout: Layout<'tree>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'tree, Message, iced::Theme, iced::Renderer>> {
        self.arriving.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'view, Message: 'view> From<SlidingPaneSwap<'view, Message>> for Element<'view, Message> {
    fn from(sliding_pane_swap: SlidingPaneSwap<'view, Message>) -> Self {
        Element::new(sliding_pane_swap)
    }
}

#[cfg(test)]
mod tests {
    use iced::Rectangle;

    use super::{SlideBand, SlideDirection};

    const BAND: Rectangle = Rectangle {
        x: 0.0,
        y: -10.0,
        width: 100.0,
        height: 50.0,
    };
    const ROW: Rectangle = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 20.0,
    };

    #[test]
    fn a_later_pane_enters_from_below() {
        assert_eq!(
            SlideDirection::between(2, 1),
            SlideDirection::EnterFromBelow
        );
        assert_eq!(
            SlideDirection::between(1, 2),
            SlideDirection::EnterFromAbove
        );
    }

    #[test]
    fn rows_slide_as_one_strip_and_leave_the_band() {
        [
            SlideDirection::EnterFromBelow,
            SlideDirection::EnterFromAbove,
        ]
        .into_iter()
        .for_each(|direction| {
            let at_start = direction.shifts(BAND, ROW, ROW, 0.0);
            let halfway = direction.shifts(BAND, ROW, ROW, 0.5);
            let at_end = direction.shifts(BAND, ROW, ROW, 1.0);
            assert!(at_start.departing_shift.abs() < f32::EPSILON);
            assert!(at_end.arriving_shift.abs() < f32::EPSILON);
            let strip_offset = at_start.arriving_shift - at_start.departing_shift;
            assert!((halfway.arriving_shift - halfway.departing_shift - strip_offset).abs() < 0.01);
            let arriving_at_start_top = ROW.y + at_start.arriving_shift;
            let departing_at_end_top = ROW.y + at_end.departing_shift;
            let band_bottom = BAND.y + BAND.height;
            match direction {
                SlideDirection::EnterFromBelow => {
                    assert!(arriving_at_start_top >= band_bottom - 0.01);
                    assert!(departing_at_end_top + ROW.height <= BAND.y + 0.01);
                }
                SlideDirection::EnterFromAbove => {
                    assert!(arriving_at_start_top + ROW.height <= BAND.y + 0.01);
                    assert!(departing_at_end_top >= band_bottom - 0.01);
                }
            }
        });
    }

    const SCREEN: Rectangle = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 1000.0,
        height: 600.0,
    };

    #[test]
    fn the_band_reaches_through_the_window_gap() {
        let bottom_regions = SlideBand::AlongBottomEdge {
            frame_depth_px: 30.0,
            float_offset_px: 4.0,
            window_gap_px: 16.0,
        }
        .regions(ROW, &SCREEN);
        assert!((bottom_regions.framed_part.y - 566.0).abs() < 0.01);
        assert!((bottom_regions.whole_band.y - 550.0).abs() < 0.01);
        assert!((bottom_regions.whole_band.height - 50.0).abs() < 0.01);
        assert!((bottom_regions.window_gap.y - 550.0).abs() < 0.01);
        let top_regions = SlideBand::AlongTopEdge {
            frame_depth_px: 30.0,
            float_offset_px: 4.0,
            window_gap_px: 16.0,
        }
        .regions(ROW, &SCREEN);
        assert!(top_regions.whole_band.y.abs() < 0.01);
        assert!((top_regions.whole_band.height - 50.0).abs() < 0.01);
        assert!((top_regions.window_gap.y - 34.0).abs() < 0.01);
    }
}
