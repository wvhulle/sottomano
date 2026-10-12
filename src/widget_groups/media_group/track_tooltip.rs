use std::time::{Duration, Instant};

use reqwest::Url;

use crate::{
    data::source::mpris::Media,
    widgets::tooltip_body::{Cell, KeyValue, Section},
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

    fn url(self, query: &str) -> String {
        let host = self.host();
        match self {
            Self::YouTubeMusic | Self::Bandcamp => {
                format!("https://{host}/search?{}", query_string("q", query))
            }
            Self::Spotify => format!("https://{host}/search/{}", path_segment(query)),
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

pub fn track_section(media: &Media) -> Section {
    let source = media
        .url
        .as_ref()
        .map(|url| KeyValue::new("Source", Cell::Link(url.to_string())));
    Section::dictionary(
        [
            ("Title", &media.title),
            ("Artist", &media.artist),
            ("Album", &media.album),
        ]
        .into_iter()
        .filter_map(|(key, value)| Some(KeyValue::new(key, value.as_deref()?)))
        .chain(source),
    )
    .titled("Track")
}

pub fn playback_section(media: &Media, now: Instant) -> Section {
    let progress = media.progress;
    let status = media.status;
    let duration = |key, duration: Option<Duration>| {
        duration.map(|duration| KeyValue::new(key, duration_text(duration)))
    };
    Section::dictionary(
        [
            KeyValue::new("Player", media.identity.as_str()),
            KeyValue::new("State", status.label()),
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

pub fn search_section(media: &Media) -> Section {
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
            .map(move |platform| KeyValue::new(platform.name(), Cell::Link(platform.url(&query))))
    });
    Section::dictionary(links).titled("Search")
}

fn duration_text(duration: Duration) -> String {
    humantime::format_duration(Duration::from_secs(duration.as_secs())).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_encodes_spaces_and_non_ascii_per_platform() {
        let query = "Felix Räuber Stay Here";
        assert_eq!(
            SearchPlatform::YouTubeMusic.url(query),
            "https://music.youtube.com/search?q=Felix+R%C3%A4uber+Stay+Here"
        );
        assert_eq!(
            SearchPlatform::Spotify.url(query),
            "https://open.spotify.com/search/Felix%20R%C3%A4uber%20Stay%20Here"
        );
    }
}
