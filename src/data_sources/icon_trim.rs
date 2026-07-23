//! Trim the transparent margin that desktop-theme app icons ship with.
//!
//! Icon themes (Colloid, Papirus, …) draw each glyph inside a padded canvas —
//! e.g. artwork occupying the central ~80% of a 24×24 box. Rendered at the
//! same nominal side as a Font Awesome glyph (which fills its em-box), the
//! themed icon then looks noticeably smaller and lighter than its neighbours.
//!
//! The QML original couldn't fix this: Quickshell served themed icons as opaque
//! `image://` URLs, so the padding couldn't be measured. odin-rs resolves the
//! actual SVG *file*, so we can parse it, measure the artwork's tight bounding
//! box, and rewrite the root `viewBox` to crop the margin — staying vector, so
//! it renders crisply at any size. The trimmed copy is cached on disk and its
//! path returned; the render path then loads it exactly like any other icon.

use std::{
    fs,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

/// Below this the margin isn't worth cropping — return the original so we don't
/// churn the cache for icons that already fill their canvas.
const MIN_MARGIN_FRACTION: f32 = 0.02;

/// Bumped whenever the trim algorithm changes, to invalidate on-disk caches
/// written by an earlier version (keyed only by icon path + mtime otherwise).
const CACHE_VERSION: u32 = 2;

/// Return a path to a margin-trimmed copy of `path`, or `path` itself when it
/// is not an SVG, can't be parsed, or already fills its canvas. Never fails: on
/// any error it falls back to the original path so the icon still renders.
pub fn trimmed(path: &Path) -> PathBuf {
    if path
        .extension()
        .is_none_or(|e| !e.eq_ignore_ascii_case("svg"))
    {
        return path.to_path_buf();
    }
    trim_svg(path).unwrap_or_else(|| path.to_path_buf())
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

/// Per-user cache for trimmed icons, under `$XDG_STATE_HOME/odin/icon-trim`.
fn cache_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    let dir = base.join("odin/icon-trim");
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
