use std::time::{Duration, Instant};

use reqwest::Url;
use sottomano_desktop_sources::{
    desktop_windows_apps::desktop_entries::{DesktopEntries, SchemeHandling},
    media_playback_audio::mpris_players::Media,
};

use crate::{
    clock::Clock,
    glyph_icons::{icon_glyph::IconGlyph, simple_icons::SimpleIcon},
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        subscribed_snapshot::SubscribedSnapshot,
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

    fn glyph(self) -> IconGlyph {
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

    fn url(self, query: &str, desktop_entries: &DesktopEntries) -> String {
        let host = self.host();
        match self {
            Self::YouTubeMusic | Self::Bandcamp => {
                format!("https://{host}/search?{}", query_string("q", query))
            }
            Self::Spotify => {
                self.spotify_link(&format!("/search/{}", path_segment(query)), desktop_entries)
            }
        }
    }

    fn spotify_link(self, path: &str, desktop_entries: &DesktopEntries) -> String {
        if desktop_entries.scheme_handling("spotify") == SchemeHandling::Handled {
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

pub fn track_section(media: &Media, desktop_entries: &DesktopEntries) -> Section {
    let source = media.url.as_ref().and_then(|url| {
        let label = url.host_str()?.to_owned();
        let link = match SearchPlatform::hosting(url) {
            Some(spotify @ SearchPlatform::Spotify) => ValueCell::BrandedLink {
                label,
                url: spotify.spotify_link(url.path(), desktop_entries),
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
    Section::new(
        "Metadata",
        dictionary_table(
            [
                ("Title", media.title.clone()),
                ("Artist", media.joined_artists()),
                ("Album", media.album.clone()),
            ]
            .into_iter()
            .filter_map(|(key, value)| Some(LabeledValue::new(key, value?)))
            .chain(source),
        ),
    )
}

pub fn playback_section(media: &Media, now: Instant) -> Section {
    let progress = media.progress;
    let status = media.status;
    let duration = |key, duration: Option<Duration>| {
        duration.map(|duration| LabeledValue::new(key, ValueCell::duration(duration)))
    };
    Section::new(
        "Playback",
        dictionary_table(
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
        ),
    )
}

pub fn search_section(media: &Media, desktop_entries: &DesktopEntries) -> Section {
    let query = media.title.as_ref().map(|title| {
        media
            .joined_artists()
            .map_or_else(|| title.clone(), |artists| format!("{artists} {title}"))
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
                        url: platform.url(&query, desktop_entries),
                        glyph: platform.glyph(),
                    },
                )
            })
    });
    Section::new("Search", dictionary_table(links))
}

pub(super) fn track_hover_card(
    player_snapshot: &SubscribedSnapshot<Media>,
    media: &Media,
    desktop_entries: &DesktopEntries,
    clock: Clock,
) -> Vec<Section> {
    player_snapshot.hover_card_with(vec![
        track_section(media, desktop_entries),
        playback_section(media, clock.instant),
        search_section(media, desktop_entries),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_encodes_spaces_and_non_ascii_per_platform() {
        let query = "Felix Räuber Stay Here";
        let desktop_entries = DesktopEntries::default();
        assert_eq!(
            SearchPlatform::YouTubeMusic.url(query, &desktop_entries),
            "https://music.youtube.com/search?q=Felix+R%C3%A4uber+Stay+Here"
        );
        assert_eq!(
            SearchPlatform::Spotify.url(query, &desktop_entries),
            "https://open.spotify.com/search/Felix%20R%C3%A4uber%20Stay%20Here"
        );
    }
}
