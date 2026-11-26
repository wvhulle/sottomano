#[cfg(test)]
mod icon_file_loading_tests;

use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use image::{ImageError, RgbaImage, imageops::crop_imm};
use tokio::fs;

use crate::{
    blocking_background_work::{Panicked, blocking_poll},
    theme_icons::icon_ink_classification::{IconDrawing, classified_svg},
};

const MIN_MARGIN_FRACTION: f32 = 0.02;

const ANTIALIAS_BLEED_FRACTION: f32 = 1.0 / 16.0;

const ALPHA_INK_THRESHOLD: u8 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconTrim {
    Trimmed,
    DesignSize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoadedIcon {
    pub drawing: IconDrawing,
    pub design_side_px: f32,
}

#[derive(Debug, thiserror::Error)]
pub enum IconLoadError {
    #[error("cannot read icon {icon_path}: {read_failure}")]
    Unreadable {
        icon_path: PathBuf,
        read_failure: io::Error,
    },
    #[error("invalid svg icon {icon_path}: {parse_failure}")]
    InvalidSvg {
        icon_path: PathBuf,
        parse_failure: usvg::Error,
    },
    #[error("cannot decode raster icon {icon_path}: {decode_failure}")]
    UndecodableRaster {
        icon_path: PathBuf,
        decode_failure: ImageError,
    },
    #[error("decoding an icon panicked")]
    DecodePanicked(#[from] Panicked),
}

pub async fn loaded_icon_file(
    icon_path: PathBuf,
    icon_trim: IconTrim,
) -> Result<LoadedIcon, IconLoadError> {
    let icon_bytes =
        fs::read(&icon_path)
            .await
            .map_err(|read_failure| IconLoadError::Unreadable {
                icon_path: icon_path.clone(),
                read_failure,
            })?;
    blocking_poll(move || decoded_icon(&icon_path, icon_bytes, icon_trim)).await
}

fn decoded_icon(
    icon_path: &Path,
    icon_bytes: Vec<u8>,
    icon_trim: IconTrim,
) -> Result<LoadedIcon, IconLoadError> {
    if is_svg_file(icon_path) {
        decoded_svg(icon_path, icon_bytes, icon_trim)
    } else {
        decoded_raster(icon_path, &icon_bytes, icon_trim)
    }
}

fn decoded_svg(
    icon_path: &Path,
    icon_bytes: Vec<u8>,
    icon_trim: IconTrim,
) -> Result<LoadedIcon, IconLoadError> {
    let svg_tree =
        usvg::Tree::from_data(&icon_bytes, &usvg::Options::default()).map_err(|parse_failure| {
            IconLoadError::InvalidSvg {
                icon_path: icon_path.to_path_buf(),
                parse_failure,
            }
        })?;
    let canvas_side_px = svg_tree.size().width().max(svg_tree.size().height());
    let framed_svg = match icon_trim {
        IconTrim::Trimmed => cropped_svg(&svg_tree),
        IconTrim::DesignSize => SvgFraming::Uncropped,
    };
    let (svg_bytes, design_side_px) = match framed_svg {
        SvgFraming::Cropped(cropped) => (cropped.svg_bytes, cropped.side_px),
        SvgFraming::Uncropped => (icon_bytes, canvas_side_px),
    };
    Ok(LoadedIcon {
        drawing: classified_svg(icon_path, &svg_tree, svg_bytes.into()),
        design_side_px,
    })
}

fn decoded_raster(
    icon_path: &Path,
    icon_bytes: &[u8],
    icon_trim: IconTrim,
) -> Result<LoadedIcon, IconLoadError> {
    let decoded_pixels = image::load_from_memory(icon_bytes)
        .map_err(|decode_failure| IconLoadError::UndecodableRaster {
            icon_path: icon_path.to_path_buf(),
            decode_failure,
        })?
        .into_rgba8();
    let framed_pixels = match icon_trim {
        IconTrim::Trimmed => trim_rgba(decoded_pixels),
        IconTrim::DesignSize => decoded_pixels,
    };
    Ok(LoadedIcon {
        design_side_px: framed_pixels.width().max(framed_pixels.height()) as f32,
        drawing: IconDrawing::Raster(Arc::new(framed_pixels)),
    })
}

#[must_use]
pub fn is_svg_file(icon_path: &Path) -> bool {
    icon_path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("svg") || extension.eq_ignore_ascii_case("svgz")
        })
}

#[must_use]
pub fn trim_rgba(image: RgbaImage) -> RgbaImage {
    let (width, height) = image.dimensions();
    match significant_ink_bounds(width, height, image.as_raw()) {
        Some(bounds) => {
            crop_imm(&image, bounds.x, bounds.y, bounds.width, bounds.height).to_image()
        }
        None => image,
    }
}

struct CroppedSvg {
    svg_bytes: Vec<u8>,
    side_px: f32,
}

enum SvgFraming {
    Cropped(CroppedSvg),
    Uncropped,
}

fn cropped_svg(tree: &usvg::Tree) -> SvgFraming {
    let size = tree.size();
    let ink_box = tree.root().abs_stroke_bounding_box();
    let bleed = ink_box.width().max(ink_box.height()) * ANTIALIAS_BLEED_FRACTION;
    let Some(crop_box) = usvg::Rect::from_ltrb(
        (ink_box.left() - bleed).max(0.0),
        (ink_box.top() - bleed).max(0.0),
        (ink_box.right() + bleed).min(size.width()),
        (ink_box.bottom() + bleed).min(size.height()),
    ) else {
        return SvgFraming::Uncropped;
    };

    let margin_x = 1.0 - crop_box.width() / size.width();
    let margin_y = 1.0 - crop_box.height() / size.height();
    if margin_x < MIN_MARGIN_FRACTION && margin_y < MIN_MARGIN_FRACTION {
        return SvgFraming::Uncropped;
    }

    match crop_view_box(tree, crop_box) {
        Some(cropped_text) => SvgFraming::Cropped(CroppedSvg {
            svg_bytes: cropped_text.into_bytes(),
            side_px: crop_box.width().max(crop_box.height()),
        }),
        None => SvgFraming::Uncropped,
    }
}

fn crop_view_box(tree: &usvg::Tree, crop_box: usvg::Rect) -> Option<String> {
    let (left, top, width, height) = (
        crop_box.left(),
        crop_box.top(),
        crop_box.width(),
        crop_box.height(),
    );
    let svg = tree.to_string(&usvg::WriteOptions::default());
    let tag_end = svg.find('>')?;
    let opening = &svg[..tag_end];
    let body = &svg[tag_end + 1..];
    let xlink = if opening.contains("xmlns:xlink") {
        " xmlns:xlink=\"http://www.w3.org/1999/xlink\""
    } else {
        ""
    };
    Some(format!(
        "<svg width=\"{width}\" height=\"{height}\" viewBox=\"{left} {top} {width} {height}\" \
         xmlns=\"http://www.w3.org/2000/svg\"{xlink}>{body}"
    ))
}

struct InkBounds {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

fn significant_ink_bounds(width: u32, height: u32, rgba: &[u8]) -> Option<InkBounds> {
    let bounds = ink_bounds(width, height, rgba)?;
    let margin_x = 1.0 - bounds.width as f32 / width as f32;
    let margin_y = 1.0 - bounds.height as f32 / height as f32;
    (margin_x >= MIN_MARGIN_FRACTION || margin_y >= MIN_MARGIN_FRACTION).then_some(bounds)
}

fn ink_bounds(width: u32, height: u32, rgba: &[u8]) -> Option<InkBounds> {
    if width == 0 || height == 0 || rgba.len() < (width as usize) * (height as usize) * 4 {
        return None;
    }
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (width, height, 0, 0);
    let mut found = false;
    for row in 0..height {
        for column in 0..width {
            let alpha = rgba[(row as usize * width as usize + column as usize) * 4 + 3];
            if alpha > ALPHA_INK_THRESHOLD {
                found = true;
                min_x = min_x.min(column);
                min_y = min_y.min(row);
                max_x = max_x.max(column);
                max_y = max_y.max(row);
            }
        }
    }
    found.then(|| InkBounds {
        x: min_x,
        y: min_y,
        width: max_x - min_x + 1,
        height: max_y - min_y + 1,
    })
}
