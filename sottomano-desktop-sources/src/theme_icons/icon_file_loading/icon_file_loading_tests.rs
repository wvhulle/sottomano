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
    let icon_path = scratch.directory_path.join("app.svg");
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
    let icon_path = scratch
        .directory_path
        .join("utilities-terminal-symbolic.svg");
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
    let icon_path = scratch.directory_path.join("app.svg");
    write_icon_with_nix_store_mtime(&icon_path, EDGE_TO_EDGE_SQUARE_SVG.as_bytes());
    assert_eq!(trimmed_svg_text(&icon_path).await, EDGE_TO_EDGE_SQUARE_SVG);
}

#[tokio::test]
async fn padded_raster_gets_cropped_pixels() {
    let scratch = ScratchIconDirectory::create("padded-raster");
    let icon_path = scratch.directory_path.join("app.png");
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
    let icon_path = scratch.directory_path.join("app.svg");
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
        loaded_icon_file(scratch.directory_path.join("absent.svg"), IconTrim::Trimmed).await,
        Err(IconLoadError::Unreadable { .. })
    ));
}

#[tokio::test]
async fn malformed_svg_reports_typed_error() {
    let scratch = ScratchIconDirectory::create("malformed-svg");
    let icon_path = scratch.directory_path.join("broken.svg");
    write_icon_with_nix_store_mtime(&icon_path, b"<svg");
    assert!(matches!(
        loaded_icon_file(icon_path, IconTrim::Trimmed).await,
        Err(IconLoadError::InvalidSvg { .. })
    ));
}
