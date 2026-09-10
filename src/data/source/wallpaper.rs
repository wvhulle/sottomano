use std::{
    collections::HashMap,
    env,
    fmt::{Display, Formatter, Result as FmtResult},
    fs, io,
    path::{Path, PathBuf},
};

use iced::{
    Subscription,
    futures::{SinkExt, StreamExt, channel::mpsc::Sender, future::ready},
    stream,
};
use icu_locale_core::Locale;
use inotify::{EventMask, Inotify, WatchMask};
use serde::Deserialize;
use tokio::time::{Duration, sleep};

use crate::data::source::{ai::read_secret, command, http};

const WATCH_RETRY: Duration = Duration::from_secs(2);
const EVENT_BUFFER_BYTES: usize = 1024;

const UNSPLASH_PHOTO_ENDPOINT: &str = "https://api.unsplash.com/photos/";
const UNSPLASH_PHOTO_PAGE: &str = "https://unsplash.com/photos/";
const UNSPLASH_ID_LENGTH: usize = 11;
const UNSPLASH_SUFFIX: &str = "-unsplash";
const DISPLAYED_IMAGE_MARKER: &str = "currently displaying: image: ";
const WALLPAPER_DAEMONS: [&str; 2] = ["awww", "swww"];
const ARTICLE_SEARCH_RADIUS_KM: u32 = 10;
const ARTICLE_CANDIDATES: u32 = 3;
const USER_AGENT: &str = "vetrata/0.1 (https://codeberg.org/wvhulle/vetrata)";
const FALLBACK_LANGUAGE: &str = "en";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position {
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Wallpaper {
    pub file_name: String,
    pub caption: Option<String>,
    pub photographer: Option<String>,
    pub photo_url: String,
    pub place: Option<String>,
    pub position: Option<Position>,
    pub article: Option<WikipediaArticle>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WikipediaArticle {
    pub title: String,
    pub url: String,
    pub summary: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Error {
    Secret,
    Fetch(http::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Secret => write!(f, "unsplash access key unreadable"),
            Error::Fetch(error) => write!(f, "{error}"),
        }
    }
}

impl From<http::Error> for Error {
    fn from(error: http::Error) -> Self {
        Error::Fetch(error)
    }
}

#[derive(Deserialize)]
struct PhotoResponse {
    description: Option<String>,
    alt_description: Option<String>,
    user: Option<ReportedPhotographer>,
    links: Option<PhotoLinks>,
    location: Option<PhotoLocation>,
}

#[derive(Deserialize)]
struct ReportedPhotographer {
    name: Option<String>,
}

#[derive(Deserialize)]
struct PhotoLinks {
    html: Option<String>,
}

#[derive(Deserialize)]
struct PhotoLocation {
    name: Option<String>,
    position: Option<ReportedPosition>,
}

#[derive(Deserialize)]
struct ReportedPosition {
    latitude: Option<f64>,
    longitude: Option<f64>,
}

#[derive(Deserialize)]
struct SearchEnvelope {
    #[serde(default)]
    query: Option<SearchResults>,
}

#[derive(Deserialize)]
struct SearchResults {
    pages: HashMap<String, SearchedPage>,
}

#[derive(Deserialize)]
struct SearchedPage {
    index: u32,
    pageid: i64,
    title: String,
    fullurl: Option<String>,
    extract: Option<String>,
}

pub fn subscription() -> Subscription<Result<String, command::Error>> {
    Subscription::run(|| {
        stream::channel(
            4,
            |mut output: Sender<Result<String, command::Error>>| async move {
                loop {
                    if let Err(e) = watch(&mut output).await {
                        tracing::warn!("wallpaper watch: {e}");
                    }
                    sleep(WATCH_RETRY).await;
                }
            },
        )
    })
}

async fn watch(output: &mut Sender<Result<String, command::Error>>) -> Result<(), io::Error> {
    let roots = cache_roots();
    if roots.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no wallpaper cache directory to watch",
        ));
    }

    let inotify = Inotify::init()?;
    let mut watches = inotify.watches();
    let mut directories = HashMap::new();
    for root in &roots {
        if let Err(e) = watches.add(root, WatchMask::CREATE) {
            tracing::warn!("wallpaper watch {}: {e}", root.display());
        }
        for version in version_directories(root) {
            match watches.add(&version, WatchMask::CLOSE_WRITE) {
                Ok(descriptor) => {
                    directories.insert(descriptor, version);
                }
                Err(e) => tracing::warn!("wallpaper watch {}: {e}", version.display()),
            }
        }
    }

    let mut changes = inotify
        .into_event_stream([0u8; EVENT_BUFFER_BYTES])?
        .take_while(|event| {
            ready(
                !event
                    .as_ref()
                    .is_ok_and(|event| event.mask.contains(EventMask::ISDIR)),
            )
        });

    output
        .send(displayed_path().await)
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::BrokenPipe, e))?;

    while let Some(event) = changes.next().await {
        let event = event?;
        let requested = directories
            .get(&event.wd)
            .zip(event.name.as_ref())
            .map(|(directory, entry)| directory.join(entry))
            .and_then(|entry| fs::read(entry).ok())
            .and_then(|cached| {
                let requested = cached.rsplit(|byte| *byte == 0).next()?;
                String::from_utf8(requested.to_vec()).ok()
            })
            .filter(|requested| Path::new(requested).is_file());
        let reading = match requested {
            Some(path) => Ok(path),
            None => displayed_path().await,
        };
        output
            .send(reading)
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::BrokenPipe, e))?;
    }
    Ok(())
}

fn cache_roots() -> Vec<PathBuf> {
    let base = env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")));
    base.into_iter()
        .flat_map(|base| WALLPAPER_DAEMONS.map(|daemon| base.join(daemon)))
        .filter(|root| root.is_dir())
        .collect()
}

fn version_directories(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect()
}

pub async fn displayed_path() -> Result<String, command::Error> {
    let mut failure = None;
    for daemon in WALLPAPER_DAEMONS {
        match command::output(daemon, &["query"]).await {
            Ok(Some(queried)) => return Ok(shown_image(&queried).unwrap_or_default().to_owned()),
            Ok(None) => {}
            Err(error) => failure = Some(error),
        }
    }
    Err(failure.unwrap_or_else(|| command::Error::Spawn {
        program: WALLPAPER_DAEMONS.join(" or "),
        detail: "not installed".to_owned(),
    }))
}

pub async fn daemon_installed() -> bool {
    for daemon in WALLPAPER_DAEMONS {
        if command::exists(daemon).await {
            return true;
        }
    }
    false
}

pub fn unresolved(path: &str, id: &str) -> Wallpaper {
    Wallpaper {
        file_name: file_name(path).to_owned(),
        caption: None,
        photographer: None,
        photo_url: format!("{UNSPLASH_PHOTO_PAGE}{id}"),
        place: None,
        position: None,
        article: None,
    }
}

pub async fn resolve(path: String, id: String, unsplash_key: &Path) -> Result<Wallpaper, Error> {
    let secret = read_secret(unsplash_key).ok_or(Error::Secret)?;
    let mut wallpaper = describe_photo(file_name(&path).to_owned(), &id, &secret).await?;
    let language = wikipedia_language();
    if let Some(position) = wallpaper.position {
        wallpaper.article = nearby_article(&language, position, wallpaper.place.as_deref()).await?;
    }
    Ok(wallpaper)
}

fn wikipedia_language() -> String {
    sys_locale::get_locales()
        .filter_map(|locale| Locale::try_from_str(&locale).ok())
        .map(|locale| locale.id.language)
        .find(|language| !language.is_unknown())
        .map_or_else(
            || FALLBACK_LANGUAGE.to_owned(),
            |language| language.as_str().to_owned(),
        )
}

fn shown_image(queried: &str) -> Option<&str> {
    queried
        .lines()
        .find_map(|line| line.split_once(DISPLAYED_IMAGE_MARKER))
        .map(|(_, path)| path.trim())
        .filter(|path| !path.is_empty())
}

pub fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

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

async fn describe_photo(file_name: String, id: &str, secret: &str) -> Result<Wallpaper, Error> {
    let authorization = format!("Client-ID {secret}");
    let url = format!("{UNSPLASH_PHOTO_ENDPOINT}{id}");
    let response: PhotoResponse = http::get_json(
        &url,
        &[
            ("Authorization", authorization.as_str()),
            ("User-Agent", USER_AGENT),
        ],
        10,
    )
    .await?;
    let location = response.location;
    Ok(Wallpaper {
        file_name,
        caption: response
            .description
            .or(response.alt_description)
            .map(|caption| caption.trim().to_owned())
            .filter(|caption| !caption.is_empty()),
        photographer: response.user.and_then(|user| user.name),
        photo_url: response
            .links
            .and_then(|links| links.html)
            .unwrap_or_else(|| format!("{UNSPLASH_PHOTO_PAGE}{id}")),
        place: location.as_ref().and_then(|location| location.name.clone()),
        position: location.and_then(|location| fix(location.position.as_ref())),
        article: None,
    })
}

fn fix(reported: Option<&ReportedPosition>) -> Option<Position> {
    let reported = reported?;
    let position = Position {
        latitude: reported.latitude?,
        longitude: reported.longitude?,
    };
    (position.latitude.abs() >= f64::EPSILON || position.longitude.abs() >= f64::EPSILON)
        .then_some(position)
}

async fn nearby_article(
    language: &str,
    position: Position,
    place: Option<&str>,
) -> Result<Option<WikipediaArticle>, Error> {
    let Position {
        latitude,
        longitude,
    } = position;
    let search = format!(
        "{} nearcoord:{ARTICLE_SEARCH_RADIUS_KM}km,{latitude},{longitude}",
        place.unwrap_or_default()
    );
    let query = form_urlencoded::Serializer::new(String::new())
        .append_pair("action", "query")
        .append_pair("generator", "search")
        .append_pair("gsrsearch", &search)
        .append_pair("gsrlimit", &ARTICLE_CANDIDATES.to_string())
        .append_pair("prop", "extracts|info")
        .append_pair("inprop", "url")
        .append_pair("exintro", "1")
        .append_pair("explaintext", "1")
        .append_pair("format", "json")
        .finish();
    let url = format!("https://{language}.wikipedia.org/w/api.php?{query}");
    let found: SearchEnvelope = http::get_json(&url, &[("User-Agent", USER_AGENT)], 10).await?;
    let Some(best) = found
        .query
        .into_iter()
        .flat_map(|results| results.pages.into_values())
        .min_by_key(|page| page.index)
    else {
        return Ok(None);
    };
    Ok(Some(WikipediaArticle {
        title: best.title,
        url: best.fullurl.unwrap_or_else(|| {
            format!(
                "https://{language}.wikipedia.org/?curid={pageid}",
                pageid = best.pageid
            )
        }),
        summary: best
            .extract
            .map(|extract| extract.trim().to_owned())
            .filter(|summary| !summary.is_empty()),
    }))
}
