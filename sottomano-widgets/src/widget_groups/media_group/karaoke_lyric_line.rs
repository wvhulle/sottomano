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
use sottomano_config::theme::font::ResolvedFont;
use sottomano_sources::media_playback_audio::{
    lrclib_synced_lyrics::{LyricSweep, TimedLyricLine},
    mpris_players::{Progress, Status},
};
use unicode_segmentation::UnicodeSegmentation;

use crate::widgets::{
    configured_theme::ThemedColor,
    text::{FontFaces, LINE_HEIGHT},
};

type MeasuredParagraph = <iced::Renderer as advanced_text::Renderer>::Paragraph;

pub const INSTRUMENTAL_BREAK_MARK: &str = "♪";

#[derive(Debug, Clone, Copy)]
pub struct PlaybackPosition {
    pub progress: Progress,
    pub status: Status,
    pub sampled_at: Instant,
}

pub struct KaraokeColors<'a> {
    pub sung: ThemedColor<'a>,
    pub unsung: ThemedColor<'a>,
}

pub struct KaraokeLyricLine<'a> {
    line: &'a TimedLyricLine,
    playback: PlaybackPosition,
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

impl<'a> KaraokeLyricLine<'a> {
    #[must_use]
    pub fn new(
        line: &'a TimedLyricLine,
        playback: PlaybackPosition,
        font: &ResolvedFont,
        colors: KaraokeColors<'a>,
    ) -> Self {
        Self {
            line,
            playback,
            font: FontFaces::from(font).regular,
            size: font.text_size_px,
            colors,
        }
    }

    fn shown_text(&self) -> &str {
        if self.line.text.is_empty() {
            INSTRUMENTAL_BREAK_MARK
        } else {
            &self.line.text
        }
    }

    fn measured_line(&self) -> MeasuredLine {
        MeasuredLine {
            text: self.shown_text().to_owned(),
            font: self.font,
            size: self.size,
        }
    }

    fn sweep_at(&self, frame: Instant) -> LyricSweep {
        let played = self
            .playback
            .progress
            .played(self.playback.status, frame)
            .unwrap_or_default();
        if self.line.text.is_empty() {
            return LyricSweep {
                segment_start: 0,
                segment_end: INSTRUMENTAL_BREAK_MARK.len(),
                segment_fraction: 1.0,
            };
        }
        self.line.sweep(played)
    }

    fn frame(&self, state: &State) -> Instant {
        state
            .latest_frame
            .map_or(self.playback.sampled_at, |latest_frame| {
                latest_frame.max(self.playback.sampled_at)
            })
    }

    fn sweeping(&self, state: &State) -> bool {
        let sweep = self.sweep_at(self.frame(state));
        self.playback.status == Status::Playing
            && (sweep.segment_end < self.shown_text().len() || sweep.segment_fraction < 1.0)
    }

    fn offset_x(&self, paragraph: &MeasuredParagraph, text_offset: usize) -> f32 {
        if text_offset == 0 && reads_right_to_left(paragraph) {
            return paragraph.min_bounds().width;
        }
        let grapheme_index = self.shown_text()[..text_offset].graphemes(true).count();
        paragraph
            .grapheme_position(0, grapheme_index)
            .map_or(0.0, |grapheme_point| grapheme_point.x)
    }

    fn sweep_x(&self, paragraph: &MeasuredParagraph, sweep: LyricSweep) -> f32 {
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

fn reads_right_to_left(paragraph: &MeasuredParagraph) -> bool {
    paragraph
        .buffer()
        .layout_runs()
        .next()
        .is_some_and(|run| run.rtl)
}

impl<'a, Message: 'a> From<KaraokeLyricLine<'a>> for Element<'a, Message> {
    fn from(lyric_line: KaraokeLyricLine<'a>) -> Self {
        Element::new(lyric_line)
    }
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for KaraokeLyricLine<'_> {
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
            bounds.x + self.sweep_x(&state.paragraph, self.sweep_at(self.frame(state)));
        let left_of_edge = Rectangle {
            width: (sweep_edge - viewport.x).max(0.0),
            ..*viewport
        };
        let right_of_edge = Rectangle {
            x: sweep_edge,
            width: (viewport.x + viewport.width - sweep_edge).max(0.0),
            ..*viewport
        };
        let (sung_clip, unsung_clip) = if reads_right_to_left(&state.paragraph) {
            (right_of_edge, left_of_edge)
        } else {
            (left_of_edge, right_of_edge)
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
        if self.sweeping(state) {
            shell.request_redraw();
        }
    }
}

#[cfg(test)]
mod tests {
    use iced::Font;

    use super::{reads_right_to_left, shaped_line};

    const ARABIC_LINE: &str = "لا تستطيع رؤية";

    #[test]
    fn arabic_reads_right_to_left_and_latin_does_not() {
        assert!(reads_right_to_left(&shaped_line(
            ARABIC_LINE,
            14.0,
            Font::DEFAULT
        )));
        assert!(!reads_right_to_left(&shaped_line(
            "Low Tide",
            14.0,
            Font::DEFAULT
        )));
    }
}
