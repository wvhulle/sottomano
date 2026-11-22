use std::time::Duration;

use futures::Stream;
use serde::Deserialize;

use crate::{
    SnapshotDelivery, SnapshotSource,
    network_connectivity::internet_connectivity_gate,
    system_protocol_clients::{dbus_desktop_buses::OnBus, http_json_client},
};

mod lrc_timed_lines;

use lrc_timed_lines::parse_lrc_lines;

const LRCLIB_GET_API: &str = "https://lrclib.net/api/get";
const LRCLIB_SEARCH_API: &str = "https://lrclib.net/api/search";
const USER_AGENT: &str = "sottomano/0.1 (https://codeberg.org/wvhulle/sottomano)";
const NOT_FOUND_STATUS: u16 = 404;
const DURATION_TOLERANCE_SECS: f64 = 2.0;
const LONGEST_ESTIMATED_LINE: Duration = Duration::from_secs(6);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LyricsQuery {
    pub artist: String,
    pub title: String,
    pub album: String,
    pub duration_secs: u64,
}

impl LyricsQuery {
    #[must_use]
    pub fn lookup_url(&self) -> String {
        let mut url_serializer = url::form_urlencoded::Serializer::new(String::new());
        url_serializer
            .append_pair("artist_name", &self.artist)
            .append_pair("track_name", &self.title);
        if !self.album.is_empty() {
            url_serializer.append_pair("album_name", &self.album);
        }
        url_serializer.append_pair("duration", &self.duration_secs.to_string());
        format!("{LRCLIB_GET_API}?{}", url_serializer.finish())
    }

    #[must_use]
    pub fn search_url(&self) -> String {
        let encoded_query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("artist_name", &self.artist)
            .append_pair("track_name", &self.title)
            .finish();
        format!("{LRCLIB_SEARCH_API}?{encoded_query}")
    }

    fn duration(&self) -> Duration {
        Duration::from_secs(self.duration_secs)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordTiming {
    Sung,
    Estimated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WordStart {
    pub starts_at: Duration,
    pub text_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimedLyricLine {
    pub starts_at: Duration,
    pub ends_at: Duration,
    pub text: String,
    pub word_starts: Vec<WordStart>,
    pub word_timing: WordTiming,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LyricSweep {
    pub segment_start: usize,
    pub segment_end: usize,
    pub segment_fraction: f32,
}

impl TimedLyricLine {
    #[must_use]
    pub fn sweep(&self, played: Duration) -> LyricSweep {
        let started_word_count = self
            .word_starts
            .partition_point(|word_start| word_start.starts_at <= played);
        let Some(current_word) = started_word_count
            .checked_sub(1)
            .and_then(|current_index| self.word_starts.get(current_index))
        else {
            return LyricSweep {
                segment_start: 0,
                segment_end: 0,
                segment_fraction: 0.0,
            };
        };
        let next_word = self.word_starts.get(started_word_count);
        let segment_ends_at = next_word.map_or(self.ends_at, |word_start| word_start.starts_at);
        let sung_part = played
            .checked_sub(current_word.starts_at)
            .expect("the current word started at or before the played time");
        let segment_fraction = match segment_ends_at.checked_sub(current_word.starts_at) {
            Some(segment_span) if !segment_span.is_zero() => {
                sung_part.as_secs_f32() / segment_span.as_secs_f32()
            }
            Some(_) | None => 1.0,
        };
        LyricSweep {
            segment_start: current_word.text_offset,
            segment_end: next_word.map_or(self.text.len(), |word_start| word_start.text_offset),
            segment_fraction: segment_fraction.clamp(0.0, 1.0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackLyrics {
    Synced(Vec<TimedLyricLine>),
    PlainOnly(String),
    Instrumental,
    Missing,
}

impl TrackLyrics {
    #[must_use]
    pub fn synced_from_lrc(lrc_text: &str) -> Self {
        Self::Synced(parse_lrc_lines(lrc_text))
    }

    #[must_use]
    pub fn current_line(&self, played: Duration) -> Option<&TimedLyricLine> {
        let Self::Synced(timed_lines) = self else {
            return None;
        };
        timed_lines.get(self.current_line_index(played)?)
    }

    #[must_use]
    pub fn current_line_index(&self, played: Duration) -> Option<usize> {
        let Self::Synced(timed_lines) = self else {
            return None;
        };
        timed_lines
            .partition_point(|timed_line| timed_line.starts_at <= played)
            .checked_sub(1)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrclibRecord {
    #[serde(default)]
    duration: f64,
    #[serde(default)]
    instrumental: bool,
    #[serde(default)]
    plain_lyrics: Option<String>,
    #[serde(default)]
    synced_lyrics: Option<String>,
}

impl LrclibRecord {
    fn duration_mismatch_secs(&self, query: &LyricsQuery) -> f64 {
        (self.duration - query.duration().as_secs_f64()).abs()
    }

    fn lyrics_timing(&self) -> LyricsTiming {
        match self.synced_lyrics.as_deref() {
            Some(lrc_text) if !lrc_text.trim().is_empty() => LyricsTiming::Synced,
            Some(_) | None => LyricsTiming::Unsynced,
        }
    }
}

fn closest_search_match(candidate_records: Vec<LrclibRecord>, query: &LyricsQuery) -> TrackLyrics {
    candidate_records
        .into_iter()
        .filter(|record| record.duration_mismatch_secs(query) <= DURATION_TOLERANCE_SECS)
        .min_by(|left, right| {
            right.lyrics_timing().cmp(&left.lyrics_timing()).then(
                left.duration_mismatch_secs(query)
                    .total_cmp(&right.duration_mismatch_secs(query)),
            )
        })
        .map_or(TrackLyrics::Missing, TrackLyrics::from)
}

impl From<LrclibRecord> for TrackLyrics {
    fn from(record: LrclibRecord) -> Self {
        let synced_lines = record
            .synced_lyrics
            .as_deref()
            .map(parse_lrc_lines)
            .filter(|timed_lines| !timed_lines.is_empty());
        let plain_text = record
            .plain_lyrics
            .filter(|plain_text| !plain_text.trim().is_empty());
        match synced_lines {
            Some(timed_lines) => Self::Synced(timed_lines),
            None => match plain_text {
                Some(plain_text) => Self::PlainOnly(plain_text),
                None if record.instrumental => Self::Instrumental,
                None => Self::Missing,
            },
        }
    }
}

async fn fetch_track_lyrics(query: &LyricsQuery) -> Result<TrackLyrics, http_json_client::Error> {
    let exact_lookup = http_json_client::get_json::<LrclibRecord>(
        &query.lookup_url(),
        &[("User-Agent", USER_AGENT)],
    )
    .await;
    match exact_lookup {
        Ok(record) => Ok(TrackLyrics::from(record)),
        Err(http_json_client::Error::Status { status, .. })
            if status == NOT_FOUND_STATUS
                || status == http_json_client::TOO_MANY_REQUESTS_STATUS
                || status >= http_json_client::SERVER_ERROR_STATUS =>
        {
            search_track_lyrics(query).await
        }
        Err(
            http_json_client::Error::HostNotResolved { .. }
            | http_json_client::Error::TimedOut { .. }
            | http_json_client::Error::Unreachable { .. },
        ) => search_track_lyrics(query).await,
        Err(lookup_failure) => Err(lookup_failure),
    }
}

async fn search_track_lyrics(query: &LyricsQuery) -> Result<TrackLyrics, http_json_client::Error> {
    let candidate_records = http_json_client::get_json::<Vec<LrclibRecord>>(
        &query.search_url(),
        &[("User-Agent", USER_AGENT)],
    )
    .await?;
    Ok(closest_search_match(candidate_records, query))
}

impl SnapshotSource for TrackLyrics {
    type Error = http_json_client::Error;
    type StreamInputs = OnBus<LyricsQuery>;

    const NAME: &'static str = "Lyrics";

    fn delivery_stream(
        OnBus {
            buses,
            inputs: query,
        }: Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        internet_connectivity_gate::while_online(buses, move || {
            let query = query.clone();
            SnapshotDelivery::single_read_deliveries(
                async move { fetch_track_lyrics(&query).await },
            )
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum LyricsTiming {
    Unsynced,
    Synced,
}

#[cfg(test)]
mod synced_lyrics_tests;
