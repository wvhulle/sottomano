use std::{collections::HashMap, path::Path};

use icu_locale_core::Locale;
use serde::Deserialize;
use uom::si::{angle::degree, f64::Angle};

use crate::{
    secret_file_reads::{AuthenticatedFetchError, read_secret},
    system_protocol_clients::http_json_client,
    wallpaper_place_article::file_name,
};

const UNSPLASH_PHOTO_ENDPOINT: &str = "https://api.unsplash.com/photos/";
const UNSPLASH_PHOTO_PAGE: &str = "https://unsplash.com/photos/";
const ARTICLE_SEARCH_RADIUS_KM: u32 = 10;
const ARTICLE_CANDIDATES: u32 = 3;
const USER_AGENT: &str = "sottomano/0.1 (https://codeberg.org/wvhulle/sottomano)";
const FALLBACK_LANGUAGE: &str = "en";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position {
    pub latitude: Angle,
    pub longitude: Angle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhotoCaption {
    Described(String),
    Undescribed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhotoCredit {
    Photographer(String),
    Uncredited,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhotoPlaceName {
    Named(String),
    Unnamed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PhotoPosition {
    Located(Position),
    Unlocated,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlaceArticle {
    Found(WikipediaArticle),
    Unmatched,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArticleSummary {
    Extract(String),
    Unextracted,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Wallpaper {
    pub file_name: String,
    pub caption: PhotoCaption,
    pub photographer: PhotoCredit,
    pub photo_url: String,
    pub place: PhotoPlaceName,
    pub position: PhotoPosition,
    pub article: PlaceArticle,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WikipediaArticle {
    pub title: String,
    pub url: String,
    pub summary: ArticleSummary,
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

#[must_use]
pub fn unresolved(path: &str, id: &str) -> Wallpaper {
    Wallpaper {
        file_name: file_name(path).to_owned(),
        caption: PhotoCaption::Undescribed,
        photographer: PhotoCredit::Uncredited,
        photo_url: format!("{UNSPLASH_PHOTO_PAGE}{id}"),
        place: PhotoPlaceName::Unnamed,
        position: PhotoPosition::Unlocated,
        article: PlaceArticle::Unmatched,
    }
}

pub async fn resolve(
    path: String,
    id: String,
    unsplash_key: &Path,
) -> Result<Wallpaper, AuthenticatedFetchError> {
    let secret = read_secret(unsplash_key)?;
    let wallpaper = describe_photo(file_name(&path).to_owned(), &id, &secret).await?;
    let language = wikipedia_language();
    let article = match wallpaper.position {
        PhotoPosition::Located(position) => best_article(
            nearby_search(&language, position, &wallpaper.place).await?,
            &language,
        ),
        PhotoPosition::Unlocated => PlaceArticle::Unmatched,
    };
    Ok(Wallpaper {
        article,
        ..wallpaper
    })
}

fn wikipedia_language() -> String {
    sys_locale::get_locales()
        .filter_map(|locale| {
            Locale::try_from_str(&locale)
                .inspect_err(|parse_failure| {
                    tracing::debug!(
                        "wallpaper article: system locale {locale} is unparsable: {parse_failure}"
                    );
                })
                .ok()
        })
        .map(|locale| locale.id.language)
        .find(|language| !language.is_unknown())
        .map_or_else(
            || FALLBACK_LANGUAGE.to_owned(),
            |language| language.as_str().to_owned(),
        )
}

async fn describe_photo(
    file_name: String,
    id: &str,
    secret: &str,
) -> Result<Wallpaper, AuthenticatedFetchError> {
    let authorization = format!("Client-ID {secret}");
    let url = format!("{UNSPLASH_PHOTO_ENDPOINT}{id}");
    let response: PhotoResponse = http_json_client::get_json(
        &url,
        &[
            ("Authorization", authorization.as_str()),
            ("User-Agent", USER_AGENT),
        ],
    )
    .await?;
    let location = response.location;
    Ok(Wallpaper {
        file_name,
        caption: response
            .description
            .or(response.alt_description)
            .map(|caption| caption.trim().to_owned())
            .filter(|caption| !caption.is_empty())
            .map_or(PhotoCaption::Undescribed, PhotoCaption::Described),
        photographer: response
            .user
            .and_then(|user| user.name)
            .map_or(PhotoCredit::Uncredited, PhotoCredit::Photographer),
        photo_url: response
            .links
            .and_then(|links| links.html)
            .unwrap_or_else(|| format!("{UNSPLASH_PHOTO_PAGE}{id}")),
        place: location
            .as_ref()
            .and_then(|location| location.name.clone())
            .map_or(PhotoPlaceName::Unnamed, PhotoPlaceName::Named),
        position: location
            .and_then(|location| location.position.as_ref().and_then(fix))
            .map_or(PhotoPosition::Unlocated, PhotoPosition::Located),
        article: PlaceArticle::Unmatched,
    })
}

fn fix(reported: &ReportedPosition) -> Option<Position> {
    let position = Position {
        latitude: Angle::new::<degree>(reported.latitude?),
        longitude: Angle::new::<degree>(reported.longitude?),
    };
    let at_null_island =
        position.latitude == Angle::default() && position.longitude == Angle::default();
    (!at_null_island).then_some(position)
}

async fn nearby_search(
    language: &str,
    position: Position,
    place: &PhotoPlaceName,
) -> Result<SearchEnvelope, AuthenticatedFetchError> {
    let Position {
        latitude,
        longitude,
    } = position;
    let nearcoord = format!(
        "nearcoord:{ARTICLE_SEARCH_RADIUS_KM}km,{},{}",
        latitude.get::<degree>(),
        longitude.get::<degree>()
    );
    let search = match place {
        PhotoPlaceName::Named(place) => format!("{place} {nearcoord}"),
        PhotoPlaceName::Unnamed => nearcoord,
    };
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
    Ok(http_json_client::get_json(&url, &[("User-Agent", USER_AGENT)]).await?)
}

fn best_article(found: SearchEnvelope, language: &str) -> PlaceArticle {
    let Some(best) = found
        .query
        .into_iter()
        .flat_map(|results| results.pages.into_values())
        .min_by_key(|page| page.index)
    else {
        return PlaceArticle::Unmatched;
    };
    PlaceArticle::Found(WikipediaArticle {
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
            .filter(|summary| !summary.is_empty())
            .map_or(ArticleSummary::Unextracted, ArticleSummary::Extract),
    })
}
