pub(super) mod slide_band_geometry;

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

use self::slide_band_geometry::{
    BandMargins, SlideBand, SlideDirection, centred_at, sliding_layout,
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

struct SwapState {
    band_margins: BandMargins,
    child_identities: Vec<u64>,
}

impl<'view, Message> SlidingPaneSwap<'view, Message> {
    fn panes(&self) -> impl Iterator<Item = &SwapPane<'view, Message>> {
        iter::once(&self.arriving).chain(self.departing.as_slice())
    }
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
