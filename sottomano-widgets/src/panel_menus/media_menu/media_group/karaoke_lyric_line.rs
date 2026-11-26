use corredo::changing_text::{KaraokeColors, KaraokeLine, LineSweep, SweepMotion};
use iced::{Element, time::Instant};
use sottomano_desktop_sources::media_playback_audio::{
    lrclib_synced_lyrics::TimedLyricLine,
    mpris_players::{Progress, Status},
};
use sottomano_toml_config::theme::font::ResolvedFont;

use crate::widgets::text::resolved_text_face;

pub const INSTRUMENTAL_BREAK_MARK: &str = "♪";

#[derive(Debug, Clone, Copy)]
pub struct PlaybackPosition {
    pub progress: Progress,
    pub status: Status,
    pub sampled_at: Instant,
}

pub struct KaraokeLyricLine<'view> {
    line: &'view TimedLyricLine,
    playback: PlaybackPosition,
    font: &'view ResolvedFont,
    colors: KaraokeColors<'view>,
}

impl<'view> KaraokeLyricLine<'view> {
    #[must_use]
    pub fn new(
        line: &'view TimedLyricLine,
        playback: PlaybackPosition,
        font: &'view ResolvedFont,
        colors: KaraokeColors<'view>,
    ) -> Self {
        Self {
            line,
            playback,
            font,
            colors,
        }
    }
}

impl<'view, Message: 'view> From<KaraokeLyricLine<'view>> for Element<'view, Message> {
    fn from(lyric_line: KaraokeLyricLine<'view>) -> Self {
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
            resolved_text_face(font),
            colors,
        )
        .into()
    }
}
