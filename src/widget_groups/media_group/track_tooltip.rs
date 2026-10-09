use std::time::{Duration, Instant};

use reqwest::Url;

use crate::{
    data::source::mpris::Media,
    widgets::tooltip_body::{Cell, Section},
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
    Section::titled(
        "Track",
        &[
            ("Title", Cell::from(&media.title)),
            ("Artist", Cell::from(&media.artist)),
            ("Album", Cell::from(&media.album)),
            (
                "Source",
                media
                    .url
                    .as_ref()
                    .map_or(Cell::Missing, |url| Cell::Link(url.to_string())),
            ),
        ],
    )
}

pub fn playback_section(media: &Media, now: Instant) -> Section {
    let progress = media.progress;
    let status = media.status;
    Section::titled(
        "Playback",
        &[
            ("Player", Some(media.identity.clone())),
            ("State", Some(status.label().to_owned())),
            ("Played", progress.played(status, now).map(duration_text)),
            (
                "Remaining",
                progress.remaining(status, now).map(duration_text),
            ),
            ("Length", progress.length().map(duration_text)),
        ],
    )
}

pub fn search_section(media: &Media) -> Section {
    let query = media.title.as_ref().map(|title| {
        media
            .artist
            .as_ref()
            .map_or_else(|| title.clone(), |artist| format!("{artist} {title}"))
    });
    let playing_on = media.url.as_ref().and_then(SearchPlatform::hosting);
    Section::titled(
        "Search",
        &SearchPlatform::ALL.map(|platform| {
            let link = query
                .as_ref()
                .filter(|_| Some(platform) != playing_on)
                .map_or(Cell::Missing, |query| Cell::Link(platform.url(query)));
            (platform.name(), link)
        }),
    )
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
