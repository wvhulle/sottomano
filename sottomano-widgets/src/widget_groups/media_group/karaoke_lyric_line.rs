use corredo::changing_text::{KaraokeColors, KaraokeLine, LineSweep, SweepMotion};
use iced::{Element, time::Instant};
use sottomano_desktop_sources::media_playback_audio::{
    lrclib_synced_lyrics::TimedLyricLine,
    mpris_players::{Progress, Status},
};
use sottomano_toml_config::theme::font::ResolvedFont;

use crate::widgets::text::text_face_of;

pub const INSTRUMENTAL_BREAK_MARK: &str = "♪";

#[derive(Debug, Clone, Copy)]
pub struct PlaybackPosition {
    pub progress: Progress,
    pub status: Status,
    pub sampled_at: Instant,
}

pub struct KaraokeLyricLine<'a> {
    line: &'a TimedLyricLine,
    playback: PlaybackPosition,
    font: &'a ResolvedFont,
    colors: KaraokeColors<'a>,
}

impl<'a> KaraokeLyricLine<'a> {
    #[must_use]
    pub fn new(
        line: &'a TimedLyricLine,
        playback: PlaybackPosition,
        font: &'a ResolvedFont,
        colors: KaraokeColors<'a>,
    ) -> Self {
        Self {
            line,
            playback,
            font,
            colors,
        }
    }
}

impl<'a, Message: 'a> From<KaraokeLyricLine<'a>> for Element<'a, Message> {
    fn from(lyric_line: KaraokeLyricLine<'a>) -> Self {
        let KaraokeLyricLine {
            line,
            playback,
            font,
            colors,
        } = lyric_line;
        let is_instrumental_break = line.text.is_empty();
        let shown_text = if is_instrumental_break {
            INSTRUMENTAL_BREAK_MARK
        } else {
            line.text.as_str()
        };
        let sweep_at = move |frame: Instant| {
            if is_instrumental_break {
                return LineSweep {
                    segment_start: 0,
                    segment_end: INSTRUMENTAL_BREAK_MARK.len(),
                    segment_fraction: 1.0,
                };
            }
            let played = playback
                .progress
                .played(playback.status, frame)
                .unwrap_or_default();
            let sweep = line.sweep(played);
            LineSweep {
                segment_start: sweep.segment_start,
                segment_end: sweep.segment_end,
                segment_fraction: sweep.segment_fraction,
            }
        };
        let motion = if playback.status == Status::Playing {
            SweepMotion::Advancing
        } else {
            SweepMotion::Held
        };
        KaraokeLine::new(
            shown_text,
            sweep_at,
            motion,
            playback.sampled_at,
            text_face_of(font),
            colors,
        )
        .into()
    }
}
