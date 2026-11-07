use iced::{
    Color, Element, Event, Font, Length, Pixels, Point, Rectangle, Size,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        renderer,
        text::{self as advanced_text, Paragraph, Renderer as _},
        widget::{Tree, tree},
    },
    alignment::Vertical,
    mouse::Cursor,
    time::Instant,
    widget::text::{Shaping, Wrapping},
    window,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    font_faces::TextFace, primitives::motion_state::MotionState, shared_metrics::LINE_HEIGHT,
    theme_colors::ThemedColor,
};

type MeasuredParagraph = <iced::Renderer as advanced_text::Renderer>::Paragraph;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineSweep {
    pub segment_start: usize,
    pub segment_end: usize,
    pub segment_fraction: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SweepMotion {
    Advancing,
    Held,
}

pub struct KaraokeColors<'a> {
    pub sung: ThemedColor<'a>,
    pub unsung: ThemedColor<'a>,
}

pub struct KaraokeLine<'a> {
    text: &'a str,
    sweep_at: Box<dyn Fn(Instant) -> LineSweep + 'a>,
    motion: SweepMotion,
    sampled_at: Instant,
    font: Font,
    size: f32,
    colors: KaraokeColors<'a>,
}

#[derive(Clone, PartialEq)]
struct MeasuredLine {
    text: String,
    font: Font,
    size: f32,
}

struct State {
    measured_line: Option<MeasuredLine>,
    paragraph: MeasuredParagraph,
    latest_frame: Option<Instant>,
}

impl<'a> KaraokeLine<'a> {
    #[must_use]
    pub fn new(
        text: &'a str,
        sweep_at: impl Fn(Instant) -> LineSweep + 'a,
        motion: SweepMotion,
        sampled_at: Instant,
        face: TextFace,
        colors: KaraokeColors<'a>,
    ) -> Self {
        Self {
            text,
            sweep_at: Box::new(sweep_at),
            motion,
            sampled_at,
            font: face.font,
            size: face.size_px,
            colors,
        }
    }

    fn shown_text(&self) -> &str {
        self.text
    }

    fn measured_line(&self) -> MeasuredLine {
        MeasuredLine {
            text: self.shown_text().to_owned(),
            font: self.font,
            size: self.size,
        }
    }

    fn frame(&self, state: &State) -> Instant {
        state.latest_frame.map_or(self.sampled_at, |latest_frame| {
            latest_frame.max(self.sampled_at)
        })
    }

    fn sweep_motion(&self, state: &State) -> MotionState {
        let sweep = (self.sweep_at)(self.frame(state));
        let unfinished =
            sweep.segment_end < self.shown_text().len() || sweep.segment_fraction < 1.0;
        match self.motion {
            SweepMotion::Advancing if unfinished => MotionState::Moving,
            _ => MotionState::Settled,
        }
    }

    fn offset_x(&self, paragraph: &MeasuredParagraph, text_offset: usize) -> f32 {
        if text_offset == 0 && text_direction(paragraph) == TextDirection::RightToLeft {
            return paragraph.min_bounds().width;
        }
        let grapheme_index = self.shown_text()[..text_offset].graphemes(true).count();
        paragraph
            .grapheme_position(0, grapheme_index)
            .map_or(0.0, |grapheme_point| grapheme_point.x)
    }

    fn sweep_x(&self, paragraph: &MeasuredParagraph, sweep: LineSweep) -> f32 {
        let segment_start_x = self.offset_x(paragraph, sweep.segment_start);
        let segment_end_x = self.offset_x(paragraph, sweep.segment_end);
        segment_start_x + (segment_end_x - segment_start_x) * sweep.segment_fraction
    }
}

fn shaped_line(content: &str, size: f32, font: Font) -> MeasuredParagraph {
    MeasuredParagraph::with_text(advanced_text::Text {
        content,
        bounds: Size::INFINITE,
        size: Pixels(size),
        line_height: LINE_HEIGHT,
        font,
        align_x: advanced_text::Alignment::Default,
        align_y: Vertical::Top,
        shaping: Shaping::Advanced,
        wrapping: Wrapping::None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextDirection {
    LeftToRight,
    RightToLeft,
}

fn text_direction(paragraph: &MeasuredParagraph) -> TextDirection {
    match paragraph.buffer().layout_runs().next() {
        Some(first_run) if first_run.rtl => TextDirection::RightToLeft,
        _ => TextDirection::LeftToRight,
    }
}

impl<'a, Message: 'a> From<KaraokeLine<'a>> for Element<'a, Message> {
    fn from(lyric_line: KaraokeLine<'a>) -> Self {
        Element::new(lyric_line)
    }
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for KaraokeLine<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State {
            measured_line: None,
            paragraph: MeasuredParagraph::default(),
            latest_frame: None,
        })
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &iced::Renderer, limits: &Limits) -> Node {
        let state = tree.state.downcast_mut::<State>();
        let measured_line = self.measured_line();
        if state.measured_line.as_ref() != Some(&measured_line) {
            state.paragraph =
                shaped_line(&measured_line.text, measured_line.size, measured_line.font);
            state.measured_line = Some(measured_line);
        }
        Node::new(limits.resolve(Length::Shrink, Length::Shrink, state.paragraph.min_bounds()))
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();
        let sweep_edge =
            bounds.x + self.sweep_x(&state.paragraph, (self.sweep_at)(self.frame(state)));
        let left_of_edge = Rectangle {
            width: (sweep_edge - viewport.x).max(0.0),
            ..*viewport
        };
        let right_of_edge = Rectangle {
            x: sweep_edge,
            width: (viewport.x + viewport.width - sweep_edge).max(0.0),
            ..*viewport
        };
        let (sung_clip, unsung_clip) = match text_direction(&state.paragraph) {
            TextDirection::RightToLeft => (right_of_edge, left_of_edge),
            TextDirection::LeftToRight => (left_of_edge, right_of_edge),
        };
        let fill_clipped = |renderer: &mut iced::Renderer, color: Color, clip: Rectangle| {
            renderer.fill_paragraph(
                &state.paragraph,
                Point::new(bounds.x, bounds.y),
                color,
                clip,
            );
        };
        fill_clipped(renderer, (self.colors.unsung)(theme), unsung_clip);
        fill_clipped(renderer, (self.colors.sung)(theme), sung_clip);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        _layout: Layout<'_>,
        _cursor: Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let Event::Window(window::Event::RedrawRequested(now)) = event else {
            return;
        };
        let state = tree.state.downcast_mut::<State>();
        state.latest_frame = Some(*now);
        if self.sweep_motion(state) == MotionState::Moving {
            shell.request_redraw();
        }
    }
}

#[cfg(test)]
mod tests {
    use iced::Font;

    use super::{TextDirection, shaped_line, text_direction};

    const ARABIC_LINE: &str = "لا تستطيع رؤية";

    #[test]
    fn arabic_reads_right_to_left_and_latin_does_not() {
        assert_eq!(
            text_direction(&shaped_line(ARABIC_LINE, 14.0, Font::DEFAULT)),
            TextDirection::RightToLeft
        );
        assert_eq!(
            text_direction(&shaped_line("Low Tide", 14.0, Font::DEFAULT)),
            TextDirection::LeftToRight
        );
    }
}
