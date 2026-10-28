use std::time::{Duration, Instant};

use reqwest::Url;
use sottomano_sources::media_playback_audio::mpris_players::Media;

use crate::{
    desktop_index::DesktopIndex,
    widgets::{
        bar_widget::Glyph,
        simple_icons::SimpleIcon,
        tooltip_body::Section,
        value_cells::{LabeledValue, ValueCell},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchPlatform {
    YouTubeMusic,
    Spotify,
    Bandcamp,
}

impl SearchPlatform {
    const ALL: [Self; 3] = [Self::YouTubeMusic, Self::Spotify, Self::Bandcamp];

    const fn name(self) -> &'static str {
        match self {
            Self::YouTubeMusic => "YouTube Music",
            Self::Spotify => "Spotify",
            Self::Bandcamp => "Bandcamp",
        }
    }

    fn glyph(self) -> Glyph {
        match self {
            Self::YouTubeMusic => SimpleIcon::YouTubeMusic.into(),
            Self::Spotify => SimpleIcon::Spotify.into(),
            Self::Bandcamp => SimpleIcon::Bandcamp.into(),
        }
    }

    const fn host(self) -> &'static str {
        match self {
            Self::YouTubeMusic => "music.youtube.com",
            Self::Spotify => "open.spotify.com",
            Self::Bandcamp => "bandcamp.com",
        }
    }

    fn hosting(url: &Url) -> Option<Self> {
        let host = url.host_str()?;
        Self::ALL
            .into_iter()
            .find(|platform| platform.host() == host)
    }

    fn url(self, query: &str, index: &DesktopIndex) -> String {
        let host = self.host();
        match self {
            Self::YouTubeMusic | Self::Bandcamp => {
                format!("https://{host}/search?{}", query_string("q", query))
            }
            Self::Spotify => self.spotify_link(&format!("/search/{}", path_segment(query)), index),
        }
    }

    fn spotify_link(self, path: &str, index: &DesktopIndex) -> String {
        if index.handles_scheme("spotify") {
            format!("spotify{}", path.replace('/', ":"))
        } else {
            format!("https://{}{path}", self.host())
        }
    }
}

fn query_string(key: &str, value: &str) -> String {
    form_urlencoded::Serializer::new(String::new())
        .append_pair(key, value)
        .finish()
}

fn path_segment(value: &str) -> String {
    form_urlencoded::byte_serialize(value.as_bytes())
        .collect::<String>()
        .replace('+', "%20")
}

pub fn track_section(media: &Media, index: &DesktopIndex) -> Section {
    let source = media.url.as_ref().and_then(|url| {
        let label = url.host_str()?.to_owned();
        let link = match SearchPlatform::hosting(url) {
            Some(spotify @ SearchPlatform::Spotify) => ValueCell::BrandedLink {
                label,
                url: spotify.spotify_link(url.path(), index),
                glyph: spotify.glyph(),
            },
            Some(platform @ (SearchPlatform::YouTubeMusic | SearchPlatform::Bandcamp)) => {
                ValueCell::BrandedLink {
                    label,
                    url: url.to_string(),
                    glyph: platform.glyph(),
                }
            }
            None => ValueCell::Link {
                label,
                url: url.to_string(),
            },
        };
        Some(LabeledValue::new("Source", link))
    });
    Section::dictionary(
        [
            ("Title", &media.title),
            ("Artist", &media.artist),
            ("Album", &media.album),
        ]
        .into_iter()
        .filter_map(|(key, value)| Some(LabeledValue::new(key, value.as_deref()?)))
        .chain(source),
    )
    .titled("Track")
}

pub fn playback_section(media: &Media, now: Instant) -> Section {
    let progress = media.progress;
    let status = media.status;
    let duration = |key, duration: Option<Duration>| {
        duration.map(|duration| LabeledValue::new(key, ValueCell::duration(duration)))
    };
    Section::dictionary(
        [
            LabeledValue::new("Player", media.identity.as_str()),
            LabeledValue::new("State", status.label()),
        ]
        .into_iter()
        .chain(
            [
                duration("Played", progress.played(status, now)),
                duration("Remaining", progress.remaining(status, now)),
                duration("Length", progress.length()),
            ]
            .into_iter()
            .flatten(),
        ),
    )
    .titled("Playback")
}

pub fn search_section(media: &Media, index: &DesktopIndex) -> Section {
    let query = media.title.as_ref().map(|title| {
        media
            .artist
            .as_ref()
            .map_or_else(|| title.clone(), |artist| format!("{artist} {title}"))
    });
    let playing_on = media.url.as_ref().and_then(SearchPlatform::hosting);
    let links = query.into_iter().flat_map(move |query| {
        SearchPlatform::ALL
            .into_iter()
            .filter(move |platform| Some(*platform) != playing_on)
            .map(move |platform| {
                LabeledValue::new(
                    platform.name(),
                    ValueCell::BrandedLink {
                        label: query.clone(),
                        url: platform.url(&query, index),
                        glyph: platform.glyph(),
                    },
                )
            })
    });
    Section::dictionary(links).titled("Search")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_encodes_spaces_and_non_ascii_per_platform() {
        let query = "Felix Räuber Stay Here";
        let index = DesktopIndex::default();
        assert_eq!(
            SearchPlatform::YouTubeMusic.url(query, &index),
            "https://music.youtube.com/search?q=Felix+R%C3%A4uber+Stay+Here"
        );
        assert_eq!(
            SearchPlatform::Spotify.url(query, &index),
            "https://open.spotify.com/search/Felix%20R%C3%A4uber%20Stay%20Here"
        );
    }
}
