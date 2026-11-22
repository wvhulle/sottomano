use std::{
    fs,
    ops::Range,
    path::Path,
    time::{Duration, Instant},
};

use iced::Size;
use webp_animation::{Encoder, EncoderOptions, EncodingConfig, EncodingType};

use crate::animation_recording::SCALE_FACTOR;

const LAST_FRAME_DURATION: Duration = Duration::from_millis(100);
const LOSSLESS_EFFORT: f32 = 75.0;
const LOSSLESS_METHOD: usize = 4;

pub(super) struct CapturedFrame {
    pub(super) content: FrameContent,
    pub(super) captured_at: Instant,
}

#[derive(Clone, Copy, PartialEq)]
struct PixelSpan {
    first: u16,
    last: u16,
}

impl PixelSpan {
    fn whole(length: u16) -> Self {
        PixelSpan {
            first: 0,
            last: length - 1,
        }
    }

    fn single(position: u16) -> Self {
        PixelSpan {
            first: position,
            last: position,
        }
    }

    fn union(self, other: Self) -> Self {
        PixelSpan {
            first: self.first.min(other.first),
            last: self.last.max(other.last),
        }
    }

    fn widened(self, margin: u16, within: Self) -> Self {
        PixelSpan {
            first: self
                .first
                .checked_sub(margin)
                .map_or(within.first, |widened_first| {
                    widened_first.max(within.first)
                }),
            last: self
                .last
                .checked_add(margin)
                .map_or(within.last, |widened_last| widened_last.min(within.last)),
        }
    }

    fn aligned_to_logical_pixels(self, within: Self) -> Self {
        let pixels_per_logical = SCALE_FACTOR as u16;
        let first = self.first - self.first % pixels_per_logical;
        let past_last = (self.last + 1).next_multiple_of(pixels_per_logical);
        PixelSpan {
            first: first.max(within.first),
            last: (past_last - 1).min(within.last),
        }
    }

    fn length(self) -> u16 {
        self.last - self.first + 1
    }

    fn contains(self, position: u16) -> bool {
        (self.first..=self.last).contains(&position)
    }

    fn byte_range(self) -> Range<usize> {
        usize::from(self.first) * 4..(usize::from(self.last) + 1) * 4
    }
}

#[derive(PartialEq)]
pub(super) struct ContentRows {
    rows: PixelSpan,
    columns: PixelSpan,
    rgba: Vec<u8>,
}

#[derive(PartialEq)]
pub(super) enum FrameContent {
    BackgroundOnly,
    Drawn(ContentRows),
}

impl FrameContent {
    pub(super) fn differing_from(background: [u8; 4], rgba: &[u8], canvas: FrameSize) -> Self {
        let row_bytes = usize::from(canvas.width) * 4;
        let Some((rows, columns)) = rgba
            .chunks_exact(row_bytes)
            .enumerate()
            .filter_map(|(row_index, row)| {
                let pixels = row.as_chunks::<4>().0;
                let first_x = pixels.iter().position(|pixel| *pixel != background)?;
                let last_x = pixels.iter().rposition(|pixel| *pixel != background)?;
                let pixel_position =
                    |position: usize| u16::try_from(position).expect("pixel fits a recording");
                Some((
                    PixelSpan::single(pixel_position(row_index)),
                    PixelSpan {
                        first: pixel_position(first_x),
                        last: pixel_position(last_x),
                    },
                ))
            })
            .reduce(|(rows, columns), (row, row_columns)| {
                (rows.union(row), columns.union(row_columns))
            })
        else {
            return Self::BackgroundOnly;
        };
        let start = usize::from(rows.first) * row_bytes;
        let end = (usize::from(rows.last) + 1) * row_bytes;
        Self::Drawn(ContentRows {
            rows,
            columns,
            rgba: rgba[start..end].to_vec(),
        })
    }
}

impl ContentRows {
    fn row(&self, row_index: u16, row_bytes: usize) -> Option<&[u8]> {
        self.rows.contains(row_index).then(|| {
            let start = usize::from(row_index - self.rows.first) * row_bytes;
            &self.rgba[start..start + row_bytes]
        })
    }
}

#[derive(Clone, Copy)]
pub(super) struct FrameSize {
    pub(super) width: u16,
    pub(super) height: u16,
}

impl From<Size> for FrameSize {
    fn from(canvas: Size) -> Self {
        let physical = |logical: f32| {
            let pixels = (logical * SCALE_FACTOR).round();
            u16::try_from(pixels as u32).expect("canvas fits a GIF")
        };
        FrameSize {
            width: physical(canvas.width),
            height: physical(canvas.height),
        }
    }
}

pub(super) struct ContentCrop {
    shown_rows: PixelSpan,
    shown_columns: PixelSpan,
    row_bytes: usize,
    background_row: Vec<u8>,
    pub(super) size: FrameSize,
}

impl ContentCrop {
    pub(super) fn around_content(
        frames: &[CapturedFrame],
        canvas: FrameSize,
        background: [u8; 4],
        margin: u16,
    ) -> Self {
        let all_rows = PixelSpan::whole(canvas.height);
        let all_columns = PixelSpan::whole(canvas.width);
        let contents = || {
            frames.iter().filter_map(|frame| match &frame.content {
                FrameContent::Drawn(content) => Some(content),
                FrameContent::BackgroundOnly => None,
            })
        };
        let shown_rows = contents()
            .map(|content| content.rows)
            .reduce(PixelSpan::union)
            .map_or(all_rows, |span| span.widened(margin, all_rows));
        let shown_columns = contents()
            .map(|content| content.columns)
            .reduce(PixelSpan::union)
            .map_or(all_columns, |span| {
                span.widened(margin, all_columns)
                    .aligned_to_logical_pixels(all_columns)
            });
        ContentCrop {
            shown_rows,
            shown_columns,
            row_bytes: usize::from(canvas.width) * 4,
            background_row: background.repeat(usize::from(canvas.width)),
            size: FrameSize {
                width: shown_columns.length(),
                height: shown_rows.length(),
            },
        }
    }

    pub(super) fn crop(&self, frame: &CapturedFrame) -> Vec<u8> {
        (self.shown_rows.first..=self.shown_rows.last)
            .flat_map(|row_index| {
                let drawn_row = match &frame.content {
                    FrameContent::Drawn(content) => content.row(row_index, self.row_bytes),
                    FrameContent::BackgroundOnly => None,
                };
                drawn_row.unwrap_or(&self.background_row)[self.shown_columns.byte_range()]
                    .iter()
                    .copied()
            })
            .collect()
    }
}

pub(super) fn encode_animation(frames: &[CapturedFrame], crop: &ContentCrop, animation: &Path) {
    let Some(first) = frames.first() else {
        return;
    };
    let Some(last) = frames.last() else {
        return;
    };
    let milliseconds_since_first = |at: Instant| {
        i32::try_from((at - first.captured_at).as_millis()).expect("recordings are short")
    };
    let options = EncoderOptions {
        encoding_config: Some(EncodingConfig {
            encoding_type: EncodingType::Lossless,
            quality: LOSSLESS_EFFORT,
            method: LOSSLESS_METHOD,
        }),
        ..EncoderOptions::default()
    };
    let encoder = Encoder::new_with_options(
        (u32::from(crop.size.width), u32::from(crop.size.height)),
        options,
    )
    .expect("cropped recordings are never empty");
    let webp_bytes = frames
        .chunk_by(|earlier, later| earlier.content == later.content)
        .map(|repeats| &repeats[0])
        .fold(encoder, |mut encoder, frame| {
            encoder
                .add_frame(
                    &crop.crop(frame),
                    milliseconds_since_first(frame.captured_at),
                )
                .expect("frames match the recording size");
            encoder
        })
        .finalize(milliseconds_since_first(
            last.captured_at + LAST_FRAME_DURATION,
        ))
        .expect("animation is encodable");
    fs::write(animation, &*webp_bytes).expect("recordings directory is writable");
}
