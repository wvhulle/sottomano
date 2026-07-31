//! Trim the transparent margin that desktop-theme app icons ship with.
//!
//! FontAwesome themes (Colloid, Papirus, …) draw each glyph inside a padded
//! canvas — e.g. artwork occupying the central ~80% of a 24×24 box. Rendered at
//! the same nominal side as a Font Awesome glyph (which fills its em-box), the
//! themed icon then looks noticeably smaller and lighter than its neighbours.
//!
//! The QML original couldn't fix this: Quickshell served themed icons as opaque
//! `image://` URLs, so the padding couldn't be measured. vetrata resolves the
//! actual SVG *file*, so we can parse it, measure the artwork's tight bounding
//! box, and rewrite the root `viewBox` to crop the margin — staying vector, so
//! it renders crisply at any size. The trimmed copy is cached on disk and its
//! path returned; the render path then loads it exactly like any other icon.

use std::{
    fs,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

const MIN_MARGIN_FRACTION: f32 = 0.02;

const ALPHA_INK_THRESHOLD: u8 = 8;

const CACHE_VERSION: u32 = 3;

pub fn trimmed(path: &Path) -> PathBuf {
    let cropped = match path.extension().and_then(|e| e.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("svg") => trim_svg(path),
        _ => trim_raster(path),
    };
    cropped.unwrap_or_else(|| path.to_path_buf())
}

pub fn trim_rgba(width: u32, height: u32, rgba: Vec<u8>) -> (u32, u32, Vec<u8>) {
    match significant_ink_bounds(width, height, &rgba) {
        Some(bounds) => (
            bounds.width,
            bounds.height,
            crop_rgba(width, &rgba, &bounds),
        ),
        None => (width, height, rgba),
    }
}

fn trim_svg(path: &Path) -> Option<PathBuf> {
    // Key the cache on path + mtime so a re-themed icon (same path, new bytes)
    // re-trims, but an unchanged one is a pure existence check — no re-parse.
    // The leading version tag invalidates every entry when the trim algorithm
    // changes, so stale results from an older algorithm aren't served.
    let mtime = fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    let key = format!("{CACHE_VERSION}:{}:{mtime}", path.display());
    let cache = cache_dir()?.join(format!("{}.svg", sha256_hex(&key)));
    if cache.exists() {
        return Some(cache);
    }

    let data = fs::read(path).ok()?;
    let tree = usvg::Tree::from_data(&data, &usvg::Options::default()).ok()?;
    let size = tree.size();

    // `abs_bounding_box` includes geometry, strokes and filter (drop-shadow)
    // regions that routinely spill outside the canvas — many themed icons draw
    // past the viewport and rely on it to clip. The *visible* artwork is that
    // bbox intersected with the canvas; trimming the raw bbox would zoom the
    // icon out by whatever the overhang happens to be (and by a different
    // amount per icon, so they'd end up inconsistent sizes).
    let bbox = tree.root().abs_bounding_box();
    let x0 = bbox.x().max(0.0);
    let y0 = bbox.y().max(0.0);
    let x1 = (bbox.x() + bbox.width()).min(size.width());
    let y1 = (bbox.y() + bbox.height()).min(size.height());
    let (vw, vh) = (x1 - x0, y1 - y0);
    if vw <= 0.0 || vh <= 0.0 {
        return None;
    }

    // Skip when the visible artwork already (near-)fills the canvas in both axes.
    let margin_x = 1.0 - vw / size.width();
    let margin_y = 1.0 - vh / size.height();
    if margin_x < MIN_MARGIN_FRACTION && margin_y < MIN_MARGIN_FRACTION {
        return None;
    }

    let cropped = crop_view_box(&tree, x0, y0, vw, vh)?;
    fs::write(&cache, cropped).ok()?;
    Some(cache)
}

/// Re-serialize `tree` with the root `<svg>`'s `viewBox` set to the artwork's
/// bounding box, cropping the padding. usvg's writer emits the root tag with
/// `width`/`height` (from the canvas) and no `viewBox`, drawing content in
/// user-space coords — so replacing the opening tag with one whose `viewBox`
/// and `width`/`height` are the bbox maps just the artwork onto the canvas.
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

fn trim_raster(path: &Path) -> Option<PathBuf> {
    let mtime = fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    let key = format!("{CACHE_VERSION}:raster:{}:{mtime}", path.display());
    let cache = cache_dir()?.join(format!("{}.png", sha256_hex(&key)));
    if cache.exists() {
        return Some(cache);
    }

    let image = image::open(path).ok()?.into_rgba8();
    let (width, height) = image.dimensions();
    let bounds = significant_ink_bounds(width, height, image.as_raw())?;
    image::imageops::crop_imm(&image, bounds.x, bounds.y, bounds.width, bounds.height)
        .to_image()
        .save(&cache)
        .ok()?;
    Some(cache)
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

fn crop_rgba(width: u32, rgba: &[u8], bounds: &InkBounds) -> Vec<u8> {
    let row_bytes = bounds.width as usize * 4;
    let mut cropped = Vec::with_capacity(row_bytes * bounds.height as usize);
    for y in bounds.y..bounds.y + bounds.height {
        let start = (y as usize * width as usize + bounds.x as usize) * 4;
        cropped.extend_from_slice(&rgba[start..start + row_bytes]);
    }
    cropped
}

fn cache_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    let dir = base.join("vetrata/icon-trim");
    fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

fn sha256_hex(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
