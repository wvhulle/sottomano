//! Firefox favicon resolution: page title -> favicon image bytes, ported from
//! odin's `DataProvider/Favicons.qml` + `scripts/firefox-favicon.nu`.
//!
//! niri exposes only a toplevel's app-id + title, never the active tab's URL
//! or favicon, so the favicon is recovered from Firefox's own profile: the
//! page title (the window title with the app-name suffix already stripped by
//! the caller) matches a `places.sqlite` history entry (title -> url), and
//! `favicons.sqlite` maps that url -> the stored favicon blob. Same title =>
//! same page => same favicon, so duplicate window titles are not a problem.
//!
//! The blob is returned in memory rather than written to disk: iced renders it
//! directly (see `widget_groups::window_group::window_list::Favicon`), so there
//! is no favicon cache file to manage. Only the WAL-locked profile DBs are
//! copied to disk, since they cannot be queried in place.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use niri_ipc::state::EventStreamState;
use rusqlite::Connection;

use super::app_rules::{self, AppRule};

/// A favicon blob plus which iced widget family decodes it. `Svg` covers both
/// plain SVG text and gzip-compressed `SVGz`; `Raster` covers PNG/ICO/etc.
#[derive(Debug, Clone)]
pub struct RawFavicon {
    pub bytes: Vec<u8>,
    pub kind: FaviconKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaviconKind {
    Raster,
    Svg,
}

/// Distinct favicon lookup keys for every Firefox window currently open.
pub fn firefox_keys(niri: &EventStreamState) -> Vec<String> {
    let mut keys: Vec<String> = niri
        .windows
        .windows
        .values()
        .filter_map(|w| {
            let app_id = w.app_id.as_deref().unwrap_or_default();
            let title = w.title.as_deref().unwrap_or_default();
            matches!(app_rules::rule_for(app_id), AppRule::Firefox)
                .then(|| AppRule::Firefox.favicon_key(title))
                .flatten()
        })
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys
}

/// Resolve favicon blobs for `titles` (Firefox history page titles). Titles
/// with no match (not in history, or no stored favicon) are simply absent
/// from the result.
pub fn resolve(titles: Vec<String>) -> HashMap<String, RawFavicon> {
    let mut out = HashMap::new();
    if titles.is_empty() {
        return out;
    }
    let Some(cache_dir) = state_dir() else {
        return out;
    };
    let Some(profile) = profile_dir() else {
        return out;
    };
    let Some(places) = fresh_copy(&profile.join("places.sqlite"), &cache_dir, "places.sqlite")
    else {
        return out;
    };
    let Some(favicons_db) = fresh_copy(
        &profile.join("favicons.sqlite"),
        &cache_dir,
        "favicons.sqlite",
    ) else {
        return out;
    };
    let Ok(places_conn) = Connection::open(&places) else {
        return out;
    };
    let Ok(favicons_conn) = Connection::open(&favicons_db) else {
        return out;
    };

    for title in titles {
        if let Some(favicon) = resolve_one(&places_conn, &favicons_conn, &title) {
            out.insert(title, favicon);
        }
    }
    out
}

fn resolve_one(places: &Connection, favicons: &Connection, title: &str) -> Option<RawFavicon> {
    let url: String = places
        .query_row(
            "SELECT url FROM moz_places WHERE title = ?1 ORDER BY last_visit_date DESC LIMIT 1",
            [title],
            |row| row.get(0),
        )
        .ok()?;

    let blob: Vec<u8> = favicons
        .query_row(
            "SELECT i.data FROM moz_pages_w_icons p JOIN moz_icons_to_pages m ON m.page_id = p.id \
             JOIN moz_icons i ON i.id = m.icon_id WHERE p.page_url = ?1 ORDER BY i.width DESC \
             LIMIT 1",
            [&url],
            |row| row.get(0),
        )
        .ok()?;
    if blob.is_empty() {
        return None;
    }

    let kind = classify(&blob);
    Some(RawFavicon { bytes: blob, kind })
}

/// The Firefox profile directory whose `places.sqlite` was modified most
/// recently — i.e. the currently running instance's profile.
fn profile_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let firefox_dir = PathBuf::from(home).join(".mozilla/firefox");
    fs::read_dir(firefox_dir)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let places = entry.path().join("places.sqlite");
            let modified = fs::metadata(&places).ok()?.modified().ok()?;
            Some((modified, entry.path()))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
}

/// Scratch directory for the transient profile-DB copies, under
/// `$XDG_STATE_HOME/vetrata/favicons`. The live DBs are WAL-locked (so callers
/// query a copy); no favicon images are cached here — they are rendered
/// straight from memory.
fn state_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"))
        })?;
    let dir = base.join("vetrata/favicons");
    fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Copy `src` into `cache_dir/name`, but only when `src` is newer than the
/// existing copy — the live DBs are WAL-locked (so callers query a copy) and
/// `places.sqlite` can be large (so avoid re-copying every poll).
fn fresh_copy(src: &Path, cache_dir: &Path, name: &str) -> Option<PathBuf> {
    if !src.exists() {
        return None;
    }
    let dst = cache_dir.join(name);
    let stale = match fs::metadata(&dst).and_then(|m| m.modified()) {
        Ok(dst_time) => fs::metadata(src)
            .and_then(|m| m.modified())
            .is_ok_and(|src_time| src_time > dst_time),
        Err(_) => true,
    };
    if stale {
        fs::copy(src, &dst).ok()?;
    }
    Some(dst)
}

/// Which iced widget family decodes a favicon blob, from its magic bytes.
/// Firefox stores raster favicons as PNG and scalable ones as SVG/XML *text*,
/// which may lead with a UTF-8 BOM, whitespace, an XML declaration, or a
/// comment before the first `<` — so skip those before sniffing rather than
/// testing only byte 0 (which mislabels BOM/newline-led SVGs as raster, making
/// iced's `image` widget render them blank). A gzip header (`1F 8B`) is treated
/// as `SVGz`: iced's `svg` widget ungzips it, whereas `image` cannot.
/// Everything else — PNG (`89 50 4E 47`, never whitespace or `<`), ICO, etc. —
/// is raster, which iced's `image` widget auto-detects from the bytes.
fn classify(blob: &[u8]) -> FaviconKind {
    let body = blob.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(blob);
    match body.trim_ascii_start() {
        [b'<', ..] | [0x1f, 0x8b, ..] => FaviconKind::Svg,
        _ => FaviconKind::Raster,
    }
}

#[cfg(test)]
mod tests {
    use super::{FaviconKind, classify};

    #[test]
    fn sniffs_svg_past_bom_and_whitespace() {
        // Real leading bytes observed in Firefox's favicons.sqlite (width=65535).
        for blob in [
            b"<svg xmlns=...".as_slice(),
            b"<?xml version=\"1.0\"?><svg/>",
            b"<!-- comment --><svg/>",
            b"\n<svg/>",
            &[0xEF, 0xBB, 0xBF, b'<', b's', b'v', b'g'],
            &[0xEF, 0xBB, 0xBF, b'\n', b'<'],
            &[0x1f, 0x8b, 0x08], // gzip-compressed SVGz
        ] {
            assert_eq!(classify(blob), FaviconKind::Svg);
        }
    }

    #[test]
    fn sniffs_raster() {
        assert_eq!(classify(&[0x89, 0x50, 0x4E, 0x47]), FaviconKind::Raster);
        assert_eq!(
            classify(&[0x00, 0x00, 0x01, 0x00, 0x10]),
            FaviconKind::Raster
        );
    }
}
