use std::{ops::RangeInclusive, slice::from_ref};

use iced::{
    Element, Event, Font, Length, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        overlay, renderer,
        text::{self as advanced_text, Paragraph},
        widget::{Operation, Tree, tree},
    },
    alignment::Vertical,
    mouse::{Cursor, Interaction},
    widget::text::{Shaping, Wrapping},
};

use crate::theming::{LINE_HEIGHT, TextFace};

type MeasuredParagraph = <iced::Renderer as advanced_text::Renderer>::Paragraph;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentGrowth {
    Fixed,
    WidestSeen,
}

pub trait WidthSource: Clone + PartialEq + 'static {
    fn first_width(&self) -> f32;

    fn content_growth(&self) -> ContentGrowth;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LabelShape {
    pub widget_name: &'static str,
    pub character_count: usize,
}

impl WidthSource for LabelShape {
    fn first_width(&self) -> f32 {
        0.0
    }

    fn content_growth(&self) -> ContentGrowth {
        ContentGrowth::WidestSeen
    }
}

pub struct ReadoutLabels<ReadoutOwner, FormatContext> {
    pub owner: ReadoutOwner,
    pub range: RangeInclusive<f32>,
    pub step: f32,
    pub format: fn(f32, FormatContext) -> String,
    pub context: FormatContext,
}

#[derive(Clone)]
pub struct MeasuredReadout<ReadoutOwner, FormatContext> {
    owner: ReadoutOwner,
    range: RangeInclusive<f32>,
    step: f32,
    context: FormatContext,
    font: Font,
    text_size_px: f32,
    format: fn(f32, FormatContext) -> String,
}

impl<ReadoutOwner, FormatContext> MeasuredReadout<ReadoutOwner, FormatContext> {
    #[must_use]
    pub fn new(labels: ReadoutLabels<ReadoutOwner, FormatContext>, face: TextFace) -> Self {
        Self {
            owner: labels.owner,
            range: labels.range,
            step: labels.step,
            context: labels.context,
            font: face.font,
            text_size_px: face.size_px,
            format: labels.format,
        }
    }
}

impl<ReadoutOwner: PartialEq, FormatContext: PartialEq> PartialEq
    for MeasuredReadout<ReadoutOwner, FormatContext>
{
    fn eq(&self, other: &Self) -> bool {
        self.owner == other.owner
            && self.range == other.range
            && self.step.to_bits() == other.step.to_bits()
            && self.context == other.context
            && self.font == other.font
            && self.text_size_px.to_bits() == other.text_size_px.to_bits()
    }
}

impl<ReadoutOwner, FormatContext: Copy> MeasuredReadout<ReadoutOwner, FormatContext> {
    fn values(&self) -> impl Iterator<Item = f32> {
        let start = *self.range.start();
        let end = *self.range.end();
        let step = self.step;
        let whole_steps = ((end - start) / step).floor();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a finite, non-negative step count fits a usize"
        )]
        let steps = if whole_steps.is_finite() && whole_steps >= 0.0 {
            whole_steps as usize
        } else {
            0
        };
        #[expect(
            clippy::cast_precision_loss,
            reason = "a readout has far fewer steps than f32 counts exactly"
        )]
        (0..=steps)
            .map(move |position| start + position as f32 * step)
            .chain([end])
    }
}

impl<ReadoutOwner: Clone + PartialEq + 'static, FormatContext: Copy + PartialEq + 'static>
    WidthSource for MeasuredReadout<ReadoutOwner, FormatContext>
{
    fn first_width(&self) -> f32 {
        self.values()
            .map(|value| {
                MeasuredParagraph::with_text(advanced_text::Text {
                    content: (self.format)(value, self.context).as_str(),
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

    fn content_growth(&self) -> ContentGrowth {
        ContentGrowth::Fixed
    }
}

struct HeldWidth<Source> {
    source: Source,
    width: f32,
}

pub struct SteadyWidthText<'a, Message, Source> {
    source: Source,
    content: Element<'a, Message>,
}

pub fn steady_width_text<'a, Message, Source: WidthSource>(
    source: Source,
    content: impl Into<Element<'a, Message>>,
) -> SteadyWidthText<'a, Message, Source> {
    SteadyWidthText {
        source,
        content: content.into(),
    }
}

impl<'a, Message: 'a, Source: WidthSource> From<SteadyWidthText<'a, Message, Source>>
    for Element<'a, Message>
{
    fn from(steady_width_text: SteadyWidthText<'a, Message, Source>) -> Self {
        Element::new(steady_width_text)
    }
}

impl<Message, Source: WidthSource> Widget<Message, iced::Theme, iced::Renderer>
    for SteadyWidthText<'_, Message, Source>
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<HeldWidth<Source>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(HeldWidth {
            source: self.source.clone(),
            width: self.source.first_width(),
        })
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        let held = tree.state.downcast_mut::<HeldWidth<Source>>();
        if held.source != self.source {
            *held = HeldWidth {
                source: self.source.clone(),
                width: self.source.first_width(),
            };
        }
        tree.diff_children(from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let width = match self.source.content_growth() {
            ContentGrowth::Fixed => tree.state.downcast_ref::<HeldWidth<Source>>().width,
            ContentGrowth::WidestSeen => {
                let natural = self
                    .content
                    .as_widget_mut()
                    .layout(&mut tree.children[0], renderer, limits)
                    .size()
                    .width;
                let held = tree.state.downcast_mut::<HeldWidth<Source>>();
                held.width = held.width.max(natural);
                held.width
            }
        };
        let content = self.content.as_widget_mut().layout(
            &mut tree.children[0],
            renderer,
            &Limits::new(Size::new(width, 0.0), Size::new(width, limits.max().height)),
        );
        Node::with_children(Size::new(width, content.size().height), vec![content])
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
            content_layout(layout),
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
        self.content.as_widget_mut().operate(
            &mut tree.children[0],
            content_layout(layout),
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
            content_layout(layout),
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            content_layout(layout),
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'overlay>(
        &'overlay mut self,
        tree: &'overlay mut Tree,
        layout: Layout<'overlay>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'overlay, Message, iced::Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout
                .children()
                .next()
                .expect("steady width text lays out its content"),
            renderer,
            viewport,
            translation,
        )
    }
}

fn content_layout(layout: Layout<'_>) -> Layout<'_> {
    layout
        .children()
        .next()
        .expect("steady width text lays out its content")
}

#[cfg(test)]
mod tests {
    use iced::Font;

    use super::{ContentGrowth, LabelShape, MeasuredReadout, ReadoutLabels, WidthSource};
    use crate::theming::TextFace;

    fn whole_number(value: f32, _: ()) -> String {
        format!("{value:.0}")
    }

    fn readout_up_to(end: f32) -> MeasuredReadout<&'static str, ()> {
        MeasuredReadout::new(
            ReadoutLabels {
                owner: "volume",
                range: 0.0..=end,
                step: 1.0,
                format: whole_number,
                context: (),
            },
            TextFace {
                font: Font::DEFAULT,
                size_px: 14.0,
            },
        )
    }

    #[test]
    fn a_label_grows_with_its_content_and_a_readout_stays_fixed() {
        let shape = LabelShape {
            widget_name: "battery",
            character_count: 3,
        };
        assert_eq!(shape.content_growth(), ContentGrowth::WidestSeen);
        assert_eq!(readout_up_to(100.0).content_growth(), ContentGrowth::Fixed);
    }

    #[test]
    fn a_wider_value_range_measures_wider() {
        assert!(readout_up_to(1000.0).first_width() > readout_up_to(9.0).first_width());
        assert!(readout_up_to(100.0) == readout_up_to(100.0));
    }
}
