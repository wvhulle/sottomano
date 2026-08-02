use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
};

use niri_ipc::state::EventStreamState;
use rusqlite::Connection;

use super::app_rules::{self, AppRule};

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

fn profile_dir() -> Option<PathBuf> {
    let home = env::var_os("HOME")?;
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

fn state_dir() -> Option<PathBuf> {
    let base = env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))?;
    let dir = base.join("vetrata/favicons");
    fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

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
        for blob in [
            b"<svg xmlns=...".as_slice(),
            b"<?xml version=\"1.0\"?><svg/>",
            b"<!-- comment --><svg/>",
            b"\n<svg/>",
            &[0xEF, 0xBB, 0xBF, b'<', b's', b'v', b'g'],
            &[0xEF, 0xBB, 0xBF, b'\n', b'<'],
            &[0x1f, 0x8b, 0x08],
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
