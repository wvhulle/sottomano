use std::{
    collections::HashMap,
    env, fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use futures::Stream;
use sottomano_compositor_model::compositor_window_state::CompositorWindowState;

use crate::{
    blocking_background_work::blocking,
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

pub async fn resolve(titles: Vec<String>) -> HashMap<String, RawFavicon> {
    let mut favicons_by_title = HashMap::new();
    if titles.is_empty() {
        return favicons_by_title;
    }
    let Some(database_copies) = blocking("favicons", fresh_database_copies).await else {
        return favicons_by_title;
    };
    let Some(places_database) = open_read_only(&database_copies.places).await else {
        return favicons_by_title;
    };
    let Some(favicons_database) = open_read_only(&database_copies.favicons).await else {
        return favicons_by_title;
    };

    for title in titles {
        if let Some(favicon) = resolve_one(
            &places_database.connection,
            &favicons_database.connection,
            &title,
        )
        .await
        {
            favicons_by_title.insert(title, favicon);
        }
    }
    favicons_by_title
}

struct ProfileDatabaseCopies {
    places: PathBuf,
    favicons: PathBuf,
}

fn fresh_database_copies() -> Option<ProfileDatabaseCopies> {
    let Some(cache_dir) = state_dir() else {
        tracing::debug!("favicons: no writable state directory");
        return None;
    };
    let Some(profile) = profile_dir() else {
        tracing::debug!("favicons: no Firefox profile under ~/.mozilla/firefox");
        return None;
    };
    let Some(places) = fresh_copy(&profile, &cache_dir, PLACES_DB) else {
        tracing::debug!(
            "favicons: cannot copy {PLACES_DB} from {}",
            profile.display()
        );
        return None;
    };
    let Some(favicons) = fresh_copy(&profile, &cache_dir, FAVICONS_DB) else {
        tracing::debug!(
            "favicons: cannot copy {FAVICONS_DB} from {}",
            profile.display()
        );
        return None;
    };
    Some(ProfileDatabaseCopies { places, favicons })
}

const PAGE_ICON_QUERY: &str = "SELECT i.data FROM moz_pages_w_icons p JOIN moz_icons_to_pages m \
                               ON m.page_id = p.id JOIN moz_icons i ON i.id = m.icon_id WHERE \
                               p.page_url = ?1 ORDER BY i.width DESC LIMIT 1";

const ORIGIN_ICON_QUERY: &str = "SELECT i.data FROM moz_pages_w_icons p JOIN moz_icons_to_pages m \
                                 ON m.page_id = p.id JOIN moz_icons i ON i.id = m.icon_id WHERE \
                                 p.page_url LIKE ?1 || '%' ORDER BY i.width DESC LIMIT 1";

const ROOT_ICON_QUERY: &str = "SELECT data FROM moz_icons WHERE root = 1 AND icon_url = ?1 || \
                               '/favicon.ico' ORDER BY width DESC LIMIT 1";

async fn resolve_one(
    places: &turso::Connection,
    favicons: &turso::Connection,
    title: &str,
) -> Option<RawFavicon> {
    let urls = candidate_urls(places, title).await;
    if urls.is_empty() {
        tracing::debug!("favicons: no history title matches {title:?}");
        return None;
    }

    let mut icon_bytes = None;
    for url in &urls {
        icon_bytes = icon_blob(favicons, PAGE_ICON_QUERY, url).await;
        if icon_bytes.is_some() {
            break;
        }
    }
    if icon_bytes.is_none()
        && let Some(origin) = urls.first().and_then(|first_url| origin_of(first_url))
    {
        icon_bytes = icon_blob(favicons, ORIGIN_ICON_QUERY, origin).await;
        if icon_bytes.is_none() {
            icon_bytes = icon_blob(favicons, ROOT_ICON_QUERY, origin).await;
        }
    }

    if let Some(bytes) = icon_bytes {
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

async fn candidate_urls(places: &turso::Connection, title: &str) -> Vec<String> {
    let mut history_rows = match places
        .query(
            "SELECT url, title FROM moz_places WHERE title = ?1 OR (title LIKE '(%' AND title \
             LIKE '%' || ?1) ORDER BY last_visit_date DESC LIMIT 32",
            [title],
        )
        .await
    {
        Ok(history_rows) => history_rows,
        Err(query_failure) => {
            tracing::debug!("favicons: history query failed: {query_failure}");
            return Vec::new();
        }
    };
    let mut matching_urls = Vec::new();
    loop {
        let history_row = match history_rows.next().await {
            Ok(Some(history_row)) => history_row,
            Ok(None) => break,
            Err(row_failure) => {
                tracing::warn!("favicons: invalid history row: {row_failure}");
                break;
            }
        };
        match (history_row.get::<String>(0), history_row.get::<String>(1)) {
            (Ok(url), Ok(stored_title)) => {
                if strip_unread_badge(&stored_title) == title {
                    matching_urls.push(url);
                }
            }
            (Err(row_failure), _) | (_, Err(row_failure)) => {
                tracing::warn!("favicons: invalid history row: {row_failure}");
            }
        }
    }
    matching_urls
}

async fn icon_blob(favicons: &turso::Connection, query: &str, parameter: &str) -> Option<Vec<u8>> {
    let first_icon_row = async { favicons.query(query, [parameter]).await?.next().await }
        .await
        .inspect_err(|query_failure| {
            tracing::warn!("favicons: icon query for {parameter}: {query_failure}");
        })
        .ok()??;
    first_icon_row
        .get::<Vec<u8>>(0)
        .inspect_err(|column_failure| {
            tracing::warn!("favicons: icon column for {parameter}: {column_failure}");
        })
        .ok()
        .filter(|icon_bytes| !icon_bytes.is_empty())
}

fn origin_of(url: &str) -> Option<&str> {
    let (scheme, rest) = url.split_once("://")?;
    let host_len = rest.find('/').unwrap_or(rest.len());
    (host_len > 0).then(|| &url[..scheme.len() + 3 + host_len])
}

fn profile_dir() -> Option<PathBuf> {
    let home = env::var_os("HOME")?;
    let firefox_dir = PathBuf::from(home).join(".mozilla/firefox");
    fs::read_dir(&firefox_dir)
        .inspect_err(|listing_failure| {
            tracing::debug!(
                "favicons: cannot list {}: {listing_failure}",
                firefox_dir.display()
            );
        })
        .ok()?
        .filter_map(|profile_entry| {
            profile_entry
                .inspect_err(|entry_failure| {
                    tracing::debug!(
                        "favicons: profile entry in {}: {entry_failure}",
                        firefox_dir.display()
                    );
                })
                .ok()
        })
        .filter_map(|entry| {
            let places = entry.path().join("places.sqlite");
            let modified = fs::metadata(&places)
                .and_then(|metadata| metadata.modified())
                .inspect_err(|metadata_failure| {
                    tracing::debug!(
                        "favicons: no history in {}: {metadata_failure}",
                        places.display()
                    );
                })
                .ok()?;
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
    fs::create_dir_all(&dir)
        .inspect_err(|create_failure| {
            tracing::warn!(
                "favicons: cannot create {}: {create_failure}",
                dir.display()
            );
        })
        .ok()?;
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

struct OpenedProfileDatabase {
    connection: turso::Connection,
    _database: turso::Database,
}

async fn open_read_only(database_file: &Path) -> Option<OpenedProfileDatabase> {
    let Some(database_path_text) = database_file.to_str() else {
        tracing::debug!(
            "favicons: non-unicode database path {}",
            database_file.display()
        );
        return None;
    };
    let opened_database = async {
        let database = turso::Builder::new_local(database_path_text)
            .read_only(true)
            .build()
            .await?;
        let connection = database.connect()?;
        Ok::<_, turso::Error>(OpenedProfileDatabase {
            connection,
            _database: database,
        })
    }
    .await;
    opened_database
        .inspect_err(|open_failure| {
            tracing::debug!(
                "favicons: cannot open {}: {open_failure}",
                database_file.display()
            );
        })
        .ok()
}

fn fresh_copy(profile: &Path, cache_dir: &Path, name: &str) -> Option<PathBuf> {
    let database = copy_when_newer(&profile.join(name), &cache_dir.join(name))?;
    let wal_name = format!("{name}{WAL_SUFFIX}");
    if copy_when_newer(&profile.join(&wal_name), &cache_dir.join(&wal_name)).is_none() {
        let stale_wal = cache_dir.join(wal_name);
        if let Err(remove_failure) = fs::remove_file(&stale_wal)
            && remove_failure.kind() != ErrorKind::NotFound
        {
            tracing::warn!(
                "favicons: cannot remove {}: {remove_failure}",
                stale_wal.display()
            );
        }
    }
    Some(database)
}

fn copy_when_newer(src: &Path, dst: &Path) -> Option<PathBuf> {
    let src_time = fs::metadata(src)
        .and_then(|metadata| metadata.modified())
        .inspect_err(|metadata_failure| {
            tracing::debug!("favicons: no {}: {metadata_failure}", src.display());
        })
        .ok()?;
    let stale = match fs::metadata(dst).and_then(|metadata| metadata.modified()) {
        Ok(dst_time) => src_time > dst_time,
        Err(metadata_failure) => {
            if metadata_failure.kind() != ErrorKind::NotFound {
                tracing::debug!("favicons: cached {}: {metadata_failure}", dst.display());
            }
            true
        }
    };
    if stale {
        fs::copy(src, dst)
            .inspect_err(|copy_failure| {
                tracing::warn!(
                    "favicons: cannot copy {} to {}: {copy_failure}",
                    src.display(),
                    dst.display()
                );
            })
            .ok()?;
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
