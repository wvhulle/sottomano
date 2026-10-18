use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
};

use niri_ipc::state::EventStreamState;
use rusqlite::Connection;

use crate::data::source::window_label::{WindowLabel, browser_page::strip_unread_badge};

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

#[must_use]
pub fn firefox_keys(niri: &EventStreamState) -> Vec<String> {
    let mut keys: Vec<String> = niri
        .windows
        .windows
        .values()
        .filter_map(|w| {
            let title = w.title.as_deref()?;
            let label = WindowLabel::classify(w.app_id.as_deref(), title, None);
            label.favicon_key().map(str::to_owned)
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
        tracing::debug!("favicons: no writable state directory");
        return out;
    };
    let Some(profile) = profile_dir() else {
        tracing::debug!("favicons: no Firefox profile under ~/.mozilla/firefox");
        return out;
    };
    let Some(places) = fresh_copy(&profile.join("places.sqlite"), &cache_dir, "places.sqlite")
    else {
        tracing::debug!(
            "favicons: cannot copy places.sqlite from {}",
            profile.display()
        );
        return out;
    };
    let Some(favicons_db) = fresh_copy(
        &profile.join("favicons.sqlite"),
        &cache_dir,
        "favicons.sqlite",
    ) else {
        tracing::debug!(
            "favicons: cannot copy favicons.sqlite from {}",
            profile.display()
        );
        return out;
    };
    let places_conn = match Connection::open(&places) {
        Ok(connection) => connection,
        Err(error) => {
            tracing::debug!("favicons: cannot open {}: {error}", places.display());
            return out;
        }
    };
    let favicons_conn = match Connection::open(&favicons_db) {
        Ok(connection) => connection,
        Err(error) => {
            tracing::debug!("favicons: cannot open {}: {error}", favicons_db.display());
            return out;
        }
    };

    for title in titles {
        if let Some(favicon) = resolve_one(&places_conn, &favicons_conn, &title) {
            out.insert(title, favicon);
        }
    }
    out
}

const PAGE_ICON_QUERY: &str = "SELECT i.data FROM moz_pages_w_icons p JOIN moz_icons_to_pages m \
                               ON m.page_id = p.id JOIN moz_icons i ON i.id = m.icon_id WHERE \
                               p.page_url = ?1 ORDER BY i.width DESC LIMIT 1";

const ORIGIN_ICON_QUERY: &str = "SELECT i.data FROM moz_pages_w_icons p JOIN moz_icons_to_pages m \
                                 ON m.page_id = p.id JOIN moz_icons i ON i.id = m.icon_id WHERE \
                                 p.page_url LIKE ?1 || '%' ORDER BY i.width DESC LIMIT 1";

const ROOT_ICON_QUERY: &str = "SELECT data FROM moz_icons WHERE root = 1 AND icon_url = ?1 || \
                               '/favicon.ico' ORDER BY width DESC LIMIT 1";

fn resolve_one(places: &Connection, favicons: &Connection, title: &str) -> Option<RawFavicon> {
    let urls = candidate_urls(places, title);
    if urls.is_empty() {
        tracing::debug!("favicons: no history title matches {title:?}");
        return None;
    }

    let blob = urls
        .iter()
        .find_map(|url| icon_blob(favicons, PAGE_ICON_QUERY, url))
        .or_else(|| {
            let origin = origin_of(urls.first()?)?;
            icon_blob(favicons, ORIGIN_ICON_QUERY, origin)
                .or_else(|| icon_blob(favicons, ROOT_ICON_QUERY, origin))
        });

    if let Some(bytes) = blob {
        let kind = classify(&bytes);
        Some(RawFavicon { bytes, kind })
    } else {
        tracing::debug!(
            "favicons: {title:?} matched {} urls but none has an icon",
            urls.len()
        );
        None
    }
}

fn candidate_urls(places: &Connection, title: &str) -> Vec<String> {
    let mut statement = match places.prepare(
        "SELECT url, title FROM moz_places WHERE title = ?1 OR (title LIKE '(%' AND title LIKE \
         '%' || ?1) ORDER BY last_visit_date DESC LIMIT 32",
    ) {
        Ok(statement) => statement,
        Err(error) => {
            tracing::debug!("favicons: history query failed: {error}");
            return Vec::new();
        }
    };
    let Ok(rows) = statement.query_map([title], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) else {
        return Vec::new();
    };
    rows.filter_map(Result::ok)
        .filter(|(_, stored)| strip_unread_badge(stored) == title)
        .map(|(url, _)| url)
        .collect()
}

fn icon_blob(favicons: &Connection, query: &str, parameter: &str) -> Option<Vec<u8>> {
    favicons
        .query_row(query, [parameter], |row| row.get::<_, Vec<u8>>(0))
        .ok()
        .filter(|blob| !blob.is_empty())
}

fn origin_of(url: &str) -> Option<&str> {
    let (scheme, rest) = url.split_once("://")?;
    let host_len = rest.find('/').unwrap_or(rest.len());
    (host_len > 0).then(|| &url[..scheme.len() + 3 + host_len])
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
