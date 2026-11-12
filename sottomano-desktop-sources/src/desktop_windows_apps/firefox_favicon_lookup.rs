use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
};

use futures::Stream;
use rusqlite::{Connection, OpenFlags};

use crate::{
    compositor_window_ipc::compositor_window_state::CompositorWindowState,
    delivery_cadence_drivers::ChangeWatchFailure,
    desktop_windows_apps::window_title_labels::{
        WindowLabel, firefox_page_titles::strip_unread_badge,
    },
    system_protocol_clients::inotify_directory_changes::{WatchTarget, changes},
};

const PLACES_DB: &str = "places.sqlite";
const FAVICONS_DB: &str = "favicons.sqlite";
const WAL_SUFFIX: &str = "-wal";

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
pub fn firefox_keys(compositor: &CompositorWindowState) -> Vec<String> {
    let mut keys: Vec<String> = compositor
        .windows
        .values()
        .filter_map(|w| {
            let label = WindowLabel::classify(w.app_id.as_deref()?, w.title.as_deref()?);
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
    let Some(places) = fresh_copy(&profile, &cache_dir, PLACES_DB) else {
        tracing::debug!(
            "favicons: cannot copy {PLACES_DB} from {}",
            profile.display()
        );
        return out;
    };
    let Some(favicons_db) = fresh_copy(&profile, &cache_dir, FAVICONS_DB) else {
        tracing::debug!(
            "favicons: cannot copy {FAVICONS_DB} from {}",
            profile.display()
        );
        return out;
    };
    let places_conn = match open_read_only(&places) {
        Ok(connection) => connection,
        Err(error) => {
            tracing::debug!("favicons: cannot open {}: {error}", places.display());
            return out;
        }
    };
    let favicons_conn = match open_read_only(&favicons_db) {
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
    let dir = base.join("sottomano/favicons");
    fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

pub fn profile_changes() -> impl Stream<Item = Result<(), ChangeWatchFailure>> + Send {
    let written = profile_dir()
        .into_iter()
        .flat_map(|profile| {
            [PLACES_DB, FAVICONS_DB].into_iter().flat_map(move |name| {
                [
                    profile.join(name),
                    profile.join(format!("{name}{WAL_SUFFIX}")),
                ]
            })
        })
        .map(WatchTarget::File)
        .collect();
    changes(written)
}

fn open_read_only(database: &Path) -> rusqlite::Result<Connection> {
    Connection::open_with_flags(
        database,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
}

fn fresh_copy(profile: &Path, cache_dir: &Path, name: &str) -> Option<PathBuf> {
    let database = copy_when_newer(&profile.join(name), &cache_dir.join(name))?;
    let wal_name = format!("{name}{WAL_SUFFIX}");
    if copy_when_newer(&profile.join(&wal_name), &cache_dir.join(&wal_name)).is_none() {
        let _ = fs::remove_file(cache_dir.join(wal_name));
    }
    Some(database)
}

fn copy_when_newer(src: &Path, dst: &Path) -> Option<PathBuf> {
    let src_time = fs::metadata(src).and_then(|m| m.modified()).ok()?;
    let stale = fs::metadata(dst)
        .and_then(|m| m.modified())
        .ok()
        .is_none_or(|dst_time| src_time > dst_time);
    if stale {
        fs::copy(src, dst).ok()?;
    }
    Some(dst.to_path_buf())
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
