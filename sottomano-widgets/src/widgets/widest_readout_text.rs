use std::{ops::RangeInclusive, slice::from_ref};

use chrono::Locale;
use iced::{
    Element, Event, Font, Length, Rectangle, Size,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        renderer,
        text::{self as advanced_text, Paragraph},
        widget::{Tree, tree},
    },
    alignment::Vertical,
    mouse::{Cursor, Interaction},
    widget::text::{Shaping, Wrapping},
};
use sottomano_toml_config::theme::font::ResolvedFont;

use crate::widgets::text::{FontFaces, LINE_HEIGHT};

type MeasuredParagraph = <iced::Renderer as advanced_text::Renderer>::Paragraph;

pub struct ReadoutLabels<Owner> {
    pub owner: Owner,
    pub range: RangeInclusive<f32>,
    pub step: f32,
    pub format: fn(f32, Locale) -> String,
    pub locale: Locale,
}

#[derive(Clone, PartialEq)]
struct MeasuredLabels<Owner> {
    owner: Owner,
    range: RangeInclusive<f32>,
    step: f32,
    locale: Locale,
    font: Font,
    text_size_px: f32,
}

struct WidestLabel<Owner> {
    measured: MeasuredLabels<Owner>,
    width: f32,
}

struct State<Owner> {
    widest: Option<WidestLabel<Owner>>,
}

pub struct WidestReadoutText<'a, Message, Owner> {
    current: Element<'a, Message>,
    measured: MeasuredLabels<Owner>,
    format: fn(f32, Locale) -> String,
}

pub fn widest_readout_text<'a, Message, Owner>(
    labels: ReadoutLabels<Owner>,
    font: &ResolvedFont,
    current: impl Into<Element<'a, Message>>,
) -> WidestReadoutText<'a, Message, Owner> {
    WidestReadoutText {
        current: current.into(),
        measured: MeasuredLabels {
            owner: labels.owner,
            range: labels.range,
            step: labels.step,
            locale: labels.locale,
            font: FontFaces::from(font).regular,
            text_size_px: font.text_size_px,
        },
        format: labels.format,
    }
}

impl<'a, Message: 'a, Owner: Clone + PartialEq + 'static>
    From<WidestReadoutText<'a, Message, Owner>> for Element<'a, Message>
{
    fn from(readout: WidestReadoutText<'a, Message, Owner>) -> Self {
        Element::new(readout)
    }
}

impl<Owner> MeasuredLabels<Owner> {
    fn values(&self) -> impl Iterator<Item = f32> {
        let start = *self.range.start();
        let end = *self.range.end();
        let step = self.step;
        let whole_steps = ((end - start) / step).floor();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let steps = if whole_steps.is_finite() && whole_steps >= 0.0 {
            whole_steps as usize
        } else {
            0
        };
        #[allow(clippy::cast_precision_loss)]
        (0..=steps)
            .map(move |position| start + position as f32 * step)
            .chain([end])
    }

    fn widest_width(&self, format: fn(f32, Locale) -> String) -> f32 {
        self.values()
            .map(|value| {
                MeasuredParagraph::with_text(advanced_text::Text {
                    content: format(value, self.locale).as_str(),
                    bounds: Size::INFINITE,
                    size: self.text_size_px.into(),
                    line_height: LINE_HEIGHT,
                    font: self.font,
                    align_x: advanced_text::Alignment::Default,
                    align_y: Vertical::Top,
                    shaping: Shaping::Advanced,
                    wrapping: Wrapping::None,
                })
                .min_width()
            })
            .fold(0.0, f32::max)
            .ceil()
    }
}

impl<Message, Owner: Clone + PartialEq + 'static> Widget<Message, iced::Theme, iced::Renderer>
    for WidestReadoutText<'_, Message, Owner>
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State<Owner>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::<Owner> { widest: None })
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.current)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(from_ref(&self.current));
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let state = tree.state.downcast_mut::<State<Owner>>();
        let width = match &state.widest {
            Some(widest) if widest.measured == self.measured => widest.width,
            _ => {
                let width = self.measured.widest_width(self.format);
                state.widest = Some(WidestLabel {
                    measured: self.measured.clone(),
                    width,
                });
                width
            }
        };
        let current = self.current.as_widget_mut().layout(
            &mut tree.children[0],
            renderer,
            &Limits::new(Size::new(width, 0.0), Size::new(width, limits.max().height)),
        );
        Node::with_children(Size::new(width, current.size().height), vec![current])
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
        if let Some(current) = layout.children().next() {
            self.current.as_widget_mut().update(
                &mut tree.children[0],
                event,
                current,
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
        if let Some(current) = layout.children().next() {
            self.current.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                current,
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
        layout
            .children()
            .next()
            .map(|current| {
                self.current.as_widget().mouse_interaction(
                    &tree.children[0],
                    current,
                    cursor,
                    viewport,
                    renderer,
                )
            })
            .unwrap_or_default()
    }
}
