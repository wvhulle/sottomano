//! Firefox favicon resolution: page title -> cached favicon image path,
//! ported from odin's `DataProvider/Favicons.qml` +
//! `scripts/firefox-favicon.nu`.
//!
//! niri exposes only a toplevel's app-id + title, never the active tab's URL
//! or favicon, so the favicon is recovered from Firefox's own profile: the
//! page title (the window title with the app-name suffix already stripped by
//! the caller) matches a `places.sqlite` history entry (title -> url), and
//! `favicons.sqlite` maps that url -> the stored favicon blob. Same title =>
//! same page => same favicon, so duplicate window titles are not a problem.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use niri_ipc::state::EventStreamState;
use rusqlite::Connection;
use sha2::{Digest, Sha256};

use super::app_rules::{self, AppRule};

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

/// Resolve favicon paths for `titles` (Firefox history page titles). Titles
/// with no match (not in history, or no stored favicon) are simply absent
/// from the result.
pub fn resolve(titles: Vec<String>) -> HashMap<String, PathBuf> {
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

    for title in titles {
        if let Some(path) = resolve_one(&places_conn, &favicons_db, &cache_dir, &title) {
            out.insert(title, path);
        }
    }
    out
}

fn resolve_one(
    places: &Connection,
    favicons_db: &Path,
    cache_dir: &Path,
    title: &str,
) -> Option<PathBuf> {
    let url: String = places
        .query_row(
            "SELECT url FROM moz_places WHERE title = ?1 ORDER BY last_visit_date DESC LIMIT 1",
            [title],
            |row| row.get(0),
        )
        .ok()?;

    let stem = cache_dir.join(sha256_hex(&url));
    if let Some(existing) = find_cached(&stem) {
        return Some(existing);
    }

    let favicons_conn = Connection::open(favicons_db).ok()?;
    let blob: Vec<u8> = favicons_conn
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

    let path = stem.with_extension(blob_ext(&blob));
    fs::write(&path, &blob).ok()?;
    Some(path)
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

/// Per-user favicon cache (and transient profile-DB copies), under
/// `$XDG_STATE_HOME/odin/favicons` — shared with odin's own QML favicon
/// cache, since the on-disk format (blob keyed by URL hash) is identical.
fn state_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"))
        })?;
    let dir = base.join("odin/favicons");
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

fn find_cached(stem: &Path) -> Option<PathBuf> {
    let parent = stem.parent()?;
    let name = stem.file_name()?;
    fs::read_dir(parent)
        .ok()?
        .filter_map(Result::ok)
        .find(|entry| entry.path().file_stem() == Some(name))
        .map(|entry| entry.path())
}

/// A file extension picked from the favicon blob's magic bytes: ICO
/// `00 00 01 00`, SVG/XML starts with `<`, else PNG (covers the `‰PNG` magic
/// and anything else — raster formats QML's `Image` loader sniffs anyway).
fn blob_ext(blob: &[u8]) -> &'static str {
    match blob {
        [0x00, 0x00, 0x01, 0x00, ..] => "ico",
        [0x3C, ..] => "svg",
        _ => "png",
    }
}

fn sha256_hex(s: &str) -> String {
    use std::fmt::Write;

    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    hasher.finalize().iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}
