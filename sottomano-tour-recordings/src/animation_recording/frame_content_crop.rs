use std::{
    fs,
    ops::Range,
    path::Path,
    time::{Duration, Instant},
};

use iced::Size;
use image::{Rgba, RgbaImage, imageops};
use sottomano_widget_scenes::scripted_session::SCALE_FACTOR;
use webp_animation::{Encoder, EncoderOptions, EncodingConfig, EncodingType};

const BAND_SAMPLE_INSET_PIXELS: usize = 80;
const LAST_FRAME_DURATION: Duration = Duration::from_millis(100);
const LOSSLESS_EFFORT: f32 = 75.0;
const LOSSLESS_METHOD: usize = 4;

pub(super) struct CapturedFrame {
    pub(super) content: FrameContent,
    pub(super) panel_band_column: Vec<[u8; 4]>,
    pub(super) captured_at: Instant,
}

impl CapturedFrame {
    fn shows_panel_band(&self, background: [u8; 4]) -> bool {
        self.panel_band_column
            .iter()
            .any(|band_pixel| *band_pixel != background)
    }
}

#[derive(Clone)]
pub(super) struct PanelBand {
    pub(super) rows: Range<u32>,
}

impl PanelBand {
    fn row_backdrop(&self, row: u32, band_column: &[[u8; 4]], background: [u8; 4]) -> [u8; 4] {
        if self.rows.contains(&row) {
            band_column[usize::try_from(row - self.rows.start).expect("band rows fit usize")]
        } else {
            background
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub(super) struct PixelBounds {
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
}

impl PixelBounds {
    fn union(self, other: Self) -> Self {
        PixelBounds {
            left: self.left.min(other.left),
            top: self.top.min(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }

    fn width(self) -> u32 {
        self.right - self.left + 1
    }

    fn height(self) -> u32 {
        self.bottom - self.top + 1
    }
}

#[derive(PartialEq)]
pub(super) enum FrameContent {
    BackgroundOnly,
    Drawn {
        bounds: PixelBounds,
        pixels: RgbaImage,
    },
}

impl FrameContent {
    pub(super) fn differing_from(
        background: [u8; 4],
        rgba: Vec<u8>,
        canvas: FrameSize,
        panel_band: &PanelBand,
    ) -> (Self, Vec<[u8; 4]>) {
        let row_length = usize::from(canvas.width) * 4;
        let band_sample_pixel = row_length - BAND_SAMPLE_INSET_PIXELS * 4;
        let panel_band_column: Vec<[u8; 4]> = panel_band
            .rows
            .clone()
            .map(|row| {
                let row_start = usize::try_from(row).expect("rows fit usize") * row_length;
                rgba[row_start + band_sample_pixel..row_start + band_sample_pixel + 4]
                    .try_into()
                    .expect("a pixel has four channels")
            })
            .collect();
        let pixel_position =
            |position: usize| u32::try_from(position).expect("pixel fits a recording");
        let Some(bounds) = rgba
            .chunks_exact(row_length)
            .enumerate()
            .filter_map(|(row_index, row)| {
                let row_backdrop = panel_band.row_backdrop(
                    pixel_position(row_index),
                    &panel_band_column,
                    background,
                );
                let row_pixels = row.as_chunks::<4>().0;
                let pixels = if panel_band.rows.contains(&pixel_position(row_index)) {
                    &row_pixels[..row_pixels.len() - BAND_SAMPLE_INSET_PIXELS]
                } else {
                    row_pixels
                };
                let first_x = pixels.iter().position(|pixel| *pixel != row_backdrop)?;
                let last_x = pixels.iter().rposition(|pixel| *pixel != row_backdrop)?;
                Some(PixelBounds {
                    left: pixel_position(first_x),
                    top: pixel_position(row_index),
                    right: pixel_position(last_x),
                    bottom: pixel_position(row_index),
                })
            })
            .reduce(PixelBounds::union)
        else {
            return (Self::BackgroundOnly, panel_band_column);
        };
        let screenshot =
            RgbaImage::from_raw(u32::from(canvas.width), u32::from(canvas.height), rgba)
                .expect("screenshots fill the canvas");
        let pixels = imageops::crop_imm(
            &screenshot,
            bounds.left,
            bounds.top,
            bounds.width(),
            bounds.height(),
        )
        .to_image();
        (Self::Drawn { bounds, pixels }, panel_band_column)
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
    shown: PixelBounds,
    background: [u8; 4],
    panel_band: PanelBand,
    pub(super) size: FrameSize,
}

impl ContentCrop {
    pub(super) fn around_content(
        frames: &[CapturedFrame],
        canvas: FrameSize,
        background: [u8; 4],
        margin: u16,
        panel_band: PanelBand,
    ) -> Self {
        let last_column = u32::from(canvas.width) - 1;
        let last_row = u32::from(canvas.height) - 1;
        let whole_canvas = PixelBounds {
            left: 0,
            top: 0,
            right: last_column,
            bottom: last_row,
        };
        let margin = u32::from(margin);
        let pixels_per_logical = SCALE_FACTOR as u32;
        let shown = frames
            .iter()
            .filter_map(|frame| match &frame.content {
                FrameContent::Drawn { bounds, .. } => Some(*bounds),
                FrameContent::BackgroundOnly => None,
            })
            .chain(
                frames
                    .iter()
                    .any(|frame| frame.shows_panel_band(background))
                    .then(|| PixelBounds {
                        left: last_column,
                        top: panel_band.rows.start,
                        right: 0,
                        bottom: panel_band.rows.end - 1,
                    }),
            )
            .reduce(PixelBounds::union)
            .map_or(whole_canvas, |drawn| {
                let left = drawn.left.max(margin) - margin;
                let right = (drawn.right + margin).min(last_column);
                PixelBounds {
                    left: left - left % pixels_per_logical,
                    top: drawn.top.max(margin) - margin,
                    right: ((right + 1).next_multiple_of(pixels_per_logical) - 1).min(last_column),
                    bottom: (drawn.bottom + margin).min(last_row),
                }
            });
        let physical = |pixels: u32| u16::try_from(pixels).expect("the crop fits the canvas");
        ContentCrop {
            shown,
            background,
            panel_band,
            size: FrameSize {
                width: physical(shown.width()),
                height: physical(shown.height()),
            },
        }
    }

    pub(super) fn crop(&self, frame: &CapturedFrame) -> Vec<u8> {
        let mut cropped = RgbaImage::from_pixel(
            self.shown.width(),
            self.shown.height(),
            Rgba(self.background),
        );
        for (band_row, band_pixel) in self.panel_band.rows.clone().zip(&frame.panel_band_column) {
            if !(self.shown.top..=self.shown.bottom).contains(&band_row) {
                continue;
            }
            let cropped_row = band_row - self.shown.top;
            for cropped_column in 0..self.shown.width() {
                cropped.put_pixel(cropped_column, cropped_row, Rgba(*band_pixel));
            }
        }
        if let FrameContent::Drawn { bounds, pixels } = &frame.content {
            imageops::replace(
                &mut cropped,
                pixels,
                i64::from(bounds.left - self.shown.left),
                i64::from(bounds.top - self.shown.top),
            );
        }
        cropped.into_raw()
    }
}

pub(super) fn encode_animation(
    frames: &[CapturedFrame],
    recording_ended_at: Instant,
    crop: &ContentCrop,
    animation: &Path,
) {
    let Some(first) = frames.first() else {
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
        .chunk_by(|earlier, later| {
            earlier.content == later.content && earlier.panel_band_column == later.panel_band_column
        })
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
            recording_ended_at + LAST_FRAME_DURATION,
        ))
        .expect("animation is encodable");
    fs::write(animation, &*webp_bytes).expect("recordings directory is writable");
}
