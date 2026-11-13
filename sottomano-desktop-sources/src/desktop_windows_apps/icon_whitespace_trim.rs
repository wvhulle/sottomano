use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
};

use image::{ImageError, RgbaImage, imageops::crop_imm};

const MIN_MARGIN_FRACTION: f32 = 0.02;

const ALPHA_INK_THRESHOLD: u8 = 8;

#[derive(Debug, Clone, PartialEq)]
pub enum TrimmedIcon {
    Svg(Arc<[u8]>),
    Raster(Arc<RgbaImage>),
}

#[derive(Debug, thiserror::Error)]
pub enum IconTrimError {
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
}

pub fn trim_icon_file(icon_path: &Path) -> Result<TrimmedIcon, IconTrimError> {
    let icon_bytes = fs::read(icon_path).map_err(|read_failure| IconTrimError::Unreadable {
        icon_path: icon_path.to_path_buf(),
        read_failure,
    })?;
    if is_svg_file(icon_path) {
        trim_svg_bytes(icon_bytes)
            .map(|svg_bytes| TrimmedIcon::Svg(svg_bytes.into()))
            .map_err(|parse_failure| IconTrimError::InvalidSvg {
                icon_path: icon_path.to_path_buf(),
                parse_failure,
            })
    } else {
        image::load_from_memory(&icon_bytes)
            .map(|decoded_image| {
                TrimmedIcon::Raster(Arc::new(trim_rgba(decoded_image.into_rgba8())))
            })
            .map_err(|decode_failure| IconTrimError::UndecodableRaster {
                icon_path: icon_path.to_path_buf(),
                decode_failure,
            })
    }
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

fn trim_svg_bytes(icon_bytes: Vec<u8>) -> Result<Vec<u8>, usvg::Error> {
    let tree = usvg::Tree::from_data(&icon_bytes, &usvg::Options::default())?;
    Ok(cropped_svg_bytes(&tree).unwrap_or(icon_bytes))
}

fn cropped_svg_bytes(tree: &usvg::Tree) -> Option<Vec<u8>> {
    let size = tree.size();
    let bbox = tree.root().abs_bounding_box();
    let x0 = bbox.x().max(0.0);
    let y0 = bbox.y().max(0.0);
    let x1 = (bbox.x() + bbox.width()).min(size.width());
    let y1 = (bbox.y() + bbox.height()).min(size.height());
    let (vw, vh) = (x1 - x0, y1 - y0);
    if vw <= 0.0 || vh <= 0.0 {
        return None;
    }

    let margin_x = 1.0 - vw / size.width();
    let margin_y = 1.0 - vh / size.height();
    if margin_x < MIN_MARGIN_FRACTION && margin_y < MIN_MARGIN_FRACTION {
        return None;
    }

    crop_view_box(tree, x0, y0, vw, vh).map(String::into_bytes)
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

    use super::{IconTrimError, TrimmedIcon, trim_icon_file};

    const PADDED_BLUE_SQUARE_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128"><rect x="40" y="40" width="20" height="20" fill="blue"/></svg>"#;
    const PADDED_RED_SQUARE_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128"><rect x="40" y="40" width="20" height="20" fill="red"/></svg>"#;
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

    fn trimmed_svg_text(icon_path: &Path) -> String {
        match trim_icon_file(icon_path).expect("svg icon trims") {
            TrimmedIcon::Svg(svg_bytes) => {
                String::from_utf8(svg_bytes.to_vec()).expect("trimmed svg is utf-8")
            }
            TrimmedIcon::Raster(_) => panic!("svg icon came back as raster"),
        }
    }

    #[test]
    fn padded_svg_gets_cropped_view_box() {
        let scratch = ScratchIconDirectory::create("padded-svg");
        let icon_path = scratch.icon_path("app.svg");
        write_icon_with_nix_store_mtime(&icon_path, PADDED_BLUE_SQUARE_SVG.as_bytes());
        assert!(trimmed_svg_text(&icon_path).contains("viewBox=\"40 40 20 20\""));
    }

    #[test]
    fn svg_without_margin_keeps_original_bytes() {
        let scratch = ScratchIconDirectory::create("edge-svg");
        let icon_path = scratch.icon_path("app.svg");
        write_icon_with_nix_store_mtime(&icon_path, EDGE_TO_EDGE_SQUARE_SVG.as_bytes());
        assert_eq!(trimmed_svg_text(&icon_path), EDGE_TO_EDGE_SQUARE_SVG);
    }

    #[test]
    fn padded_raster_gets_cropped_pixels() {
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
        match trim_icon_file(&icon_path).expect("png icon trims") {
            TrimmedIcon::Raster(cropped_image) => assert_eq!(cropped_image.dimensions(), (18, 10)),
            TrimmedIcon::Svg(_) => panic!("png icon came back as svg"),
        }
    }

    #[test]
    fn replaced_icon_at_same_path_and_mtime_yields_new_content() {
        let scratch = ScratchIconDirectory::create("replaced-svg");
        let icon_path = scratch.icon_path("app.svg");
        write_icon_with_nix_store_mtime(&icon_path, PADDED_BLUE_SQUARE_SVG.as_bytes());
        let first_trim = trimmed_svg_text(&icon_path);
        write_icon_with_nix_store_mtime(&icon_path, PADDED_RED_SQUARE_SVG.as_bytes());
        let second_trim = trimmed_svg_text(&icon_path);
        assert_ne!(first_trim, second_trim);
    }

    #[test]
    fn unreadable_icon_reports_typed_error() {
        let scratch = ScratchIconDirectory::create("missing-icon");
        assert!(matches!(
            trim_icon_file(&scratch.icon_path("absent.svg")),
            Err(IconTrimError::Unreadable { .. })
        ));
    }

    #[test]
    fn malformed_svg_reports_typed_error() {
        let scratch = ScratchIconDirectory::create("malformed-svg");
        let icon_path = scratch.icon_path("broken.svg");
        write_icon_with_nix_store_mtime(&icon_path, b"<svg");
        assert!(matches!(
            trim_icon_file(&icon_path),
            Err(IconTrimError::InvalidSvg { .. })
        ));
    }
}
