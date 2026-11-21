pub mod unsplash_photo_article;

use std::{
    collections::{HashMap, HashSet},
    env,
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
};

use futures::{Stream, StreamExt, future::ready, stream};
use futures_rx::RxExt;
use inotify::{Event, EventMask, Inotify, WatchDescriptor, WatchMask, Watches};

use crate::{
    backing_service_availability::Availability,
    system_protocol_clients::{protocol_peer_reconnects::subscribed, shell_command_output},
};

const EVENT_BUFFER_BYTES: usize = 1024;

const UNSPLASH_ID_LENGTH: usize = 11;
const UNSPLASH_SUFFIX: &str = "-unsplash";
const DISPLAYED_IMAGE_MARKER: &str = "currently displaying: image: ";
const WALLPAPER_DAEMONS: [&str; 2] = ["awww", "swww"];

#[derive(Clone, Copy)]
enum WatchHealth {
    Watching,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayedWallpaper {
    ImageFile(String),
    NoImageFile,
}

pub fn displayed()
-> impl Stream<Item = Result<DisplayedWallpaper, shell_command_output::Error>> + Send {
    subscribed((), |()| {
        let watched = watch();
        async move {
            match watched {
                Ok(events) => Ok(events),
                Err(failure) => Err(shell_command_output::Error::Spawn {
                    program: "wallpaper watch".to_owned(),
                    detail: failure.to_string(),
                }),
            }
        }
    })
}

fn watch()
-> io::Result<impl Stream<Item = Result<DisplayedWallpaper, shell_command_output::Error>> + Send> {
    let Some(base) = cache_base() else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no cache directory to watch for wallpaper daemons",
        ));
    };
    let inotify = Inotify::init()?;
    let layout = CacheLayout::watched(&mut inotify.watches(), &base)?;
    let changes = inotify
        .into_event_stream([0u8; EVENT_BUFFER_BYTES])?
        .map(move |event| event.map(|event| layout.change(&event)))
        .scan(WatchHealth::Watching, |health, change| {
            ready(match (*health, change) {
                (WatchHealth::Failed, _) | (WatchHealth::Watching, Ok(CacheChange::Layout)) => None,
                (WatchHealth::Watching, Ok(CacheChange::Wallpaper(lookup))) => Some(Ok(lookup)),
                (WatchHealth::Watching, Err(watch_failure)) => {
                    *health = WatchHealth::Failed;
                    Some(Err(watch_failure))
                }
            })
        });
    Ok(stream::iter([Ok(WallpaperLookup::QueryDaemon)])
        .chain(changes)
        .switch_map(|lookup| {
            stream::once(async move {
                match lookup {
                    Ok(lookup) => wallpaper_path(lookup).await,
                    Err(watch_failure) => Err(shell_command_output::Error::Spawn {
                        program: "wallpaper watch".to_owned(),
                        detail: watch_failure.to_string(),
                    }),
                }
            })
        }))
}

struct CacheLayout {
    base: WatchDescriptor,
    roots: HashSet<WatchDescriptor>,
    versions: HashMap<WatchDescriptor, PathBuf>,
}

enum CacheChange {
    Wallpaper(WallpaperLookup),
    Layout,
}

impl CacheChange {}

enum WallpaperLookup {
    Cached(String),
    QueryDaemon,
}

impl CacheLayout {
    fn watched(watches: &mut Watches, base: &Path) -> io::Result<Self> {
        let mut layout = Self {
            base: watches.add(base, WatchMask::CREATE)?,
            roots: HashSet::new(),
            versions: HashMap::new(),
        };
        WALLPAPER_DAEMONS
            .map(|daemon| base.join(daemon))
            .iter()
            .filter(|root| root.is_dir())
            .for_each(|root| layout.watch_root(watches, root));
        Ok(layout)
    }

    fn watch_root(&mut self, watches: &mut Watches, root: &Path) {
        match watches.add(root, WatchMask::CREATE) {
            Ok(descriptor) => {
                self.roots.insert(descriptor);
            }
            Err(e) => tracing::warn!("wallpaper watch {}: {e}", root.display()),
        }
        version_directories(root).into_iter().for_each(|version| {
            match watches.add(&version, WatchMask::CLOSE_WRITE) {
                Ok(descriptor) => {
                    self.versions.insert(descriptor, version);
                }
                Err(e) => tracing::warn!("wallpaper watch {}: {e}", version.display()),
            }
        });
    }

    fn change(&self, event: &Event<OsString>) -> CacheChange {
        let reshaped = event.mask.contains(EventMask::ISDIR)
            && (self.roots.contains(&event.wd)
                || event.wd == self.base
                    && event
                        .name
                        .as_ref()
                        .is_some_and(|name| WALLPAPER_DAEMONS.iter().any(|daemon| name == daemon)));
        if reshaped {
            return CacheChange::Layout;
        }
        CacheChange::Wallpaper(self.requested_lookup(event))
    }

    fn requested_lookup(&self, event: &Event<OsString>) -> WallpaperLookup {
        let Some(directory) = self.versions.get(&event.wd) else {
            return WallpaperLookup::QueryDaemon;
        };
        let Some(Ok(cached)) = event
            .name
            .as_ref()
            .map(|name| fs::read(directory.join(name)))
        else {
            return WallpaperLookup::QueryDaemon;
        };
        let Some(Ok(requested)) = cached
            .rsplit(|byte| *byte == 0)
            .next()
            .map(|bytes| String::from_utf8(bytes.to_vec()))
        else {
            return WallpaperLookup::QueryDaemon;
        };
        if Path::new(&requested).is_file() {
            WallpaperLookup::Cached(requested)
        } else {
            WallpaperLookup::QueryDaemon
        }
    }
}

async fn wallpaper_path(
    lookup: WallpaperLookup,
) -> Result<DisplayedWallpaper, shell_command_output::Error> {
    match lookup {
        WallpaperLookup::Cached(path) => Ok(DisplayedWallpaper::ImageFile(path)),
        WallpaperLookup::QueryDaemon => displayed_wallpaper().await,
    }
}

fn cache_base() -> Option<PathBuf> {
    env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .filter(|base| base.is_dir())
}

fn version_directories(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .filter_map(|listed_entry| {
            listed_entry
                .inspect_err(|list_failure| {
                    tracing::debug!(
                        "wallpaper watch: listing {} failed: {list_failure}",
                        root.display()
                    );
                })
                .ok()
        })
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect()
}

async fn displayed_wallpaper() -> Result<DisplayedWallpaper, shell_command_output::Error> {
    let mut failure = None;
    for daemon in WALLPAPER_DAEMONS {
        match shell_command_output::output(daemon, &["query"]).await {
            Ok(queried) => {
                return Ok(
                    shown_image(&queried).map_or(DisplayedWallpaper::NoImageFile, |path| {
                        DisplayedWallpaper::ImageFile(path.to_owned())
                    }),
                );
            }
            Err(shell_command_output::Error::NotInstalled { .. }) => {}
            Err(error) => failure = Some(error),
        }
    }
    Err(
        failure.unwrap_or_else(|| shell_command_output::Error::NotInstalled {
            program: WALLPAPER_DAEMONS.join(" or "),
        }),
    )
}

pub async fn daemon_availability() -> Availability {
    for daemon in WALLPAPER_DAEMONS {
        if shell_command_output::program_availability(daemon).await == Availability::Available {
            return Availability::Available;
        }
    }
    Availability::Unavailable
}

fn shown_image(queried: &str) -> Option<&str> {
    queried
        .lines()
        .find_map(|line| line.split_once(DISPLAYED_IMAGE_MARKER))
        .map(|(_, path)| path.trim())
        .filter(|path| !path.is_empty())
}

#[must_use]
pub fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

#[must_use]
pub fn photo_id(path: &str) -> Option<String> {
    let file = file_name(path);
    let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
    let credited = stem.strip_suffix(UNSPLASH_SUFFIX)?;
    if !credited.is_ascii() {
        return None;
    }
    let (photographer, id) = credited.split_at(credited.len().checked_sub(UNSPLASH_ID_LENGTH)?);
    let identifier_like = id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
    (photographer.ends_with('-') && identifier_like).then(|| id.to_owned())
}
