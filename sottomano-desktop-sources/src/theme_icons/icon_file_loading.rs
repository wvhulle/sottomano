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
    let x0 = (ink_box.x() - bleed).max(0.0);
    let y0 = (ink_box.y() - bleed).max(0.0);
    let x1 = (ink_box.x() + ink_box.width() + bleed).min(size.width());
    let y1 = (ink_box.y() + ink_box.height() + bleed).min(size.height());
    let (vw, vh) = (x1 - x0, y1 - y0);
    if vw <= 0.0 || vh <= 0.0 {
        return SvgFraming::Uncropped;
    }

    let margin_x = 1.0 - vw / size.width();
    let margin_y = 1.0 - vh / size.height();
    if margin_x < MIN_MARGIN_FRACTION && margin_y < MIN_MARGIN_FRACTION {
        return SvgFraming::Uncropped;
    }

    match crop_view_box(tree, x0, y0, vw, vh) {
        Some(cropped_text) => SvgFraming::Cropped(CroppedSvg {
            svg_bytes: cropped_text.into_bytes(),
            side_px: vw.max(vh),
        }),
        None => SvgFraming::Uncropped,
    }
}

fn crop_view_box(tree: &usvg::Tree, x: f32, y: f32, w: f32, h: f32) -> Option<String> {
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
        "<svg width=\"{w}\" height=\"{h}\" viewBox=\"{x} {y} {w} {h}\" \
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
    for y in 0..height {
        for x in 0..width {
            let alpha = rgba[(y as usize * width as usize + x as usize) * 4 + 3];
            if alpha > ALPHA_INK_THRESHOLD {
                found = true;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
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

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        path::{Path, PathBuf},
        process,
        time::{Duration, UNIX_EPOCH},
    };

    use image::{Rgba, RgbaImage};

    use super::{IconLoadError, IconTrim, LoadedIcon, loaded_icon_file};
    use crate::theme_icons::icon_ink_classification::IconDrawing;

    const PADDED_BLUE_SQUARE_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128"><rect x="40" y="40" width="20" height="20" fill="blue"/></svg>"#;
    const PADDED_RED_SQUARE_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128"><rect x="40" y="40" width="20" height="20" fill="red"/></svg>"#;
    const PADDED_STROKED_SQUARE_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128"><rect x="40" y="40" width="20" height="20" fill="none" stroke="black" stroke-width="8"/></svg>"#;
    const EDGE_TO_EDGE_SQUARE_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><rect width="16" height="16" fill="blue"/></svg>"#;

    struct ScratchIconDirectory {
        directory_path: PathBuf,
    }

    impl ScratchIconDirectory {
        fn create(test_name: &str) -> Self {
            let directory_path =
                env::temp_dir().join(format!("sottomano-icon-trim-{test_name}-{}", process::id()));
            fs::create_dir_all(&directory_path).expect("scratch icon directory is creatable");
            Self { directory_path }
        }

        fn icon_path(&self, file_name: &str) -> PathBuf {
            self.directory_path.join(file_name)
        }
    }

    impl Drop for ScratchIconDirectory {
        fn drop(&mut self) {
            if let Err(removal_failure) = fs::remove_dir_all(&self.directory_path) {
                eprintln!(
                    "cannot remove {}: {removal_failure}",
                    self.directory_path.display()
                );
            }
        }
    }

    fn write_icon_with_nix_store_mtime(icon_path: &Path, icon_contents: &[u8]) {
        fs::write(icon_path, icon_contents).expect("scratch icon is writable");
        fs::File::options()
            .write(true)
            .open(icon_path)
            .and_then(|icon_file| icon_file.set_modified(UNIX_EPOCH + Duration::from_secs(1)))
            .expect("scratch icon mtime is settable");
    }

    async fn trimmed_icon(icon_path: &Path) -> LoadedIcon {
        loaded_icon_file(icon_path.to_path_buf(), IconTrim::Trimmed)
            .await
            .expect("icon loads")
    }

    async fn trimmed_svg_text(icon_path: &Path) -> String {
        match trimmed_icon(icon_path).await.drawing {
            IconDrawing::ColouredSvg(svg_bytes) | IconDrawing::MonochromeSvg(svg_bytes) => {
                String::from_utf8(svg_bytes.to_vec()).expect("trimmed svg is utf-8")
            }
            IconDrawing::Raster(_) => panic!("svg icon came back as raster"),
        }
    }

    #[tokio::test]
    async fn padded_svg_gets_cropped_view_box() {
        let scratch = ScratchIconDirectory::create("padded-svg");
        let icon_path = scratch.icon_path("app.svg");
        write_icon_with_nix_store_mtime(&icon_path, PADDED_BLUE_SQUARE_SVG.as_bytes());
        assert!(
            trimmed_svg_text(&icon_path)
                .await
                .contains("viewBox=\"38.75 38.75 22.5 22.5\"")
        );
        assert!((trimmed_icon(&icon_path).await.design_side_px - 22.5).abs() < f32::EPSILON);
    }

    #[tokio::test]
    async fn cropped_svg_keeps_the_full_width_of_outer_strokes() {
        let scratch = ScratchIconDirectory::create("stroked-svg");
        let icon_path = scratch.icon_path("utilities-terminal-symbolic.svg");
        write_icon_with_nix_store_mtime(&icon_path, PADDED_STROKED_SQUARE_SVG.as_bytes());
        assert!(
            trimmed_svg_text(&icon_path)
                .await
                .contains("viewBox=\"34.25 34.25 31.5 31.5\"")
        );
    }

    #[tokio::test]
    async fn svg_without_margin_keeps_original_bytes() {
        let scratch = ScratchIconDirectory::create("edge-svg");
        let icon_path = scratch.icon_path("app.svg");
        write_icon_with_nix_store_mtime(&icon_path, EDGE_TO_EDGE_SQUARE_SVG.as_bytes());
        assert_eq!(trimmed_svg_text(&icon_path).await, EDGE_TO_EDGE_SQUARE_SVG);
    }

    #[tokio::test]
    async fn padded_raster_gets_cropped_pixels() {
        let scratch = ScratchIconDirectory::create("padded-raster");
        let icon_path = scratch.icon_path("app.png");
        let mut padded_image = RgbaImage::new(48, 48);
        for pixel_y in 10..20 {
            for pixel_x in 12..30 {
                padded_image.put_pixel(pixel_x, pixel_y, Rgba([255, 0, 0, 255]));
            }
        }
        padded_image
            .save(&icon_path)
            .expect("scratch png is writable");
        match trimmed_icon(&icon_path).await.drawing {
            IconDrawing::Raster(cropped_image) => assert_eq!(cropped_image.dimensions(), (18, 10)),
            IconDrawing::ColouredSvg(_) | IconDrawing::MonochromeSvg(_) => {
                panic!("png icon came back as svg")
            }
        }
    }

    #[tokio::test]
    async fn replaced_icon_at_same_path_and_mtime_yields_new_content() {
        let scratch = ScratchIconDirectory::create("replaced-svg");
        let icon_path = scratch.icon_path("app.svg");
        write_icon_with_nix_store_mtime(&icon_path, PADDED_BLUE_SQUARE_SVG.as_bytes());
        let first_trim = trimmed_svg_text(&icon_path).await;
        write_icon_with_nix_store_mtime(&icon_path, PADDED_RED_SQUARE_SVG.as_bytes());
        let second_trim = trimmed_svg_text(&icon_path).await;
        assert_ne!(first_trim, second_trim);
    }

    #[tokio::test]
    async fn unreadable_icon_reports_typed_error() {
        let scratch = ScratchIconDirectory::create("missing-icon");
        assert!(matches!(
            loaded_icon_file(scratch.icon_path("absent.svg"), IconTrim::Trimmed).await,
            Err(IconLoadError::Unreadable { .. })
        ));
    }

    #[tokio::test]
    async fn malformed_svg_reports_typed_error() {
        let scratch = ScratchIconDirectory::create("malformed-svg");
        let icon_path = scratch.icon_path("broken.svg");
        write_icon_with_nix_store_mtime(&icon_path, b"<svg");
        assert!(matches!(
            loaded_icon_file(icon_path, IconTrim::Trimmed).await,
            Err(IconLoadError::InvalidSvg { .. })
        ));
    }
}
