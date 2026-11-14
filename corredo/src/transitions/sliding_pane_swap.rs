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

pub struct SwapPane<'a, Message> {
    pub identity: u64,
    pub slide_order: u8,
    pub content: Element<'a, Message>,
}

pub enum DepartingPane<'a, Message> {
    Absent,
    Departing {
        pane: SwapPane<'a, Message>,
        slide_progress: f32,
    },
}

impl<'a, Message> DepartingPane<'a, Message> {
    fn as_slice(&self) -> &[SwapPane<'a, Message>] {
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
    },
    AlongBottomEdge {
        frame_depth_px: f32,
        float_offset_px: f32,
    },
}

struct BandRegions {
    whole_band: Rectangle,
    framed_part: Rectangle,
    floating_gap: Rectangle,
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
            },
            Self::AlongTopEdge {
                frame_depth_px,
                float_offset_px,
            } => {
                let band_top = viewport.y;
                BandRegions {
                    whole_band: horizontal_strip(band_top, float_offset_px + frame_depth_px),
                    framed_part: horizontal_strip(band_top + float_offset_px, frame_depth_px),
                    floating_gap: horizontal_strip(band_top, float_offset_px),
                }
            }
            Self::AlongBottomEdge {
                frame_depth_px,
                float_offset_px,
            } => {
                let band_top = viewport.y + viewport.height - float_offset_px - frame_depth_px;
                BandRegions {
                    whole_band: horizontal_strip(band_top, float_offset_px + frame_depth_px),
                    framed_part: horizontal_strip(band_top, frame_depth_px),
                    floating_gap: horizontal_strip(band_top + frame_depth_px, float_offset_px),
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
        match self {
            Self::EnterFromBelow => SlideShifts {
                departing_shift: -(departing_bounds.y + departing_bounds.height - band.y)
                    * progress,
                arriving_shift: (band_bottom - arriving_bounds.y) * (1.0 - progress),
            },
            Self::EnterFromAbove => SlideShifts {
                departing_shift: (band_bottom - departing_bounds.y) * progress,
                arriving_shift: -(arriving_bounds.y + arriving_bounds.height - band.y)
                    * (1.0 - progress),
            },
        }
    }
}

struct FadeTarget {
    region: Rectangle,
    restored_fill: Color,
}

pub struct SlidingPaneSwap<'a, Message> {
    arriving: SwapPane<'a, Message>,
    departing: DepartingPane<'a, Message>,
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
pub fn sliding_pane_swap<'a, Message: 'a>(
    arriving: SwapPane<'a, Message>,
    departing: DepartingPane<'a, Message>,
) -> SlidingPaneSwap<'a, Message> {
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

impl<'a, Message> SlidingPaneSwap<'a, Message> {
    fn panes(&self) -> impl Iterator<Item = &SwapPane<'a, Message>> {
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

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        self.arriving.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message: 'a> From<SlidingPaneSwap<'a, Message>> for Element<'a, Message> {
    fn from(sliding_pane_swap: SlidingPaneSwap<'a, Message>) -> Self {
        Element::new(sliding_pane_swap)
    }
}

#[cfg(test)]
mod tests {
    use iced::Rectangle;

    use super::{SlideDirection, SlideShifts};

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
    fn rows_start_and_end_just_past_the_band_edges() {
        let entering_from_below = SlideDirection::EnterFromBelow.shifts(BAND, ROW, ROW, 0.0);
        assert!((ROW.y + entering_from_below.arriving_shift - (BAND.y + BAND.height)).abs() < 0.01);
        assert!(entering_from_below.departing_shift.abs() < f32::EPSILON);
        let left_upwards: SlideShifts = SlideDirection::EnterFromBelow.shifts(BAND, ROW, ROW, 1.0);
        assert!((ROW.y + ROW.height + left_upwards.departing_shift - BAND.y).abs() < 0.01);
        assert!(left_upwards.arriving_shift.abs() < f32::EPSILON);
        let entering_from_above = SlideDirection::EnterFromAbove.shifts(BAND, ROW, ROW, 0.0);
        assert!((ROW.y + ROW.height + entering_from_above.arriving_shift - BAND.y).abs() < 0.01);
        let left_downwards = SlideDirection::EnterFromAbove.shifts(BAND, ROW, ROW, 1.0);
        assert!((ROW.y + left_downwards.departing_shift - (BAND.y + BAND.height)).abs() < 0.01);
    }
}
