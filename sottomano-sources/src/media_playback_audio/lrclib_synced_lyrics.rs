use std::{iter, time::Duration};

use futures::Stream;
use serde::Deserialize;

use crate::{SnapshotDelivery, SnapshotSource, system_protocol_clients::http_json_client};

const FETCH_TIMEOUT_SECS: u64 = 10;
const LRCLIB_GET_API: &str = "https://lrclib.net/api/get";
const LRCLIB_SEARCH_API: &str = "https://lrclib.net/api/search";
const USER_AGENT: &str = "sottomano/0.1 (https://codeberg.org/wvhulle/sottomano)";
const NOT_FOUND_STATUS: u16 = 404;
const RETRY_DELAYS: [Duration; 3] = [
    Duration::from_secs(2),
    Duration::from_secs(10),
    Duration::from_secs(30),
];
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
        let mut url_serializer = form_urlencoded::Serializer::new(String::new());
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
        let encoded_query = form_urlencoded::Serializer::new(String::new())
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
        let segment_span = segment_ends_at.saturating_sub(current_word.starts_at);
        let segment_fraction = if segment_span.is_zero() {
            1.0
        } else {
            played.saturating_sub(current_word.starts_at).as_secs_f32() / segment_span.as_secs_f32()
        };
        LyricSweep {
            segment_start: current_word.text_offset,
            segment_end: next_word.map_or(self.text.len(), |word_start| word_start.text_offset),
            segment_fraction: segment_fraction.clamp(0.0, 1.0),
        }
    }

    fn with_completed_word_timing(self, next_line_start: Option<Duration>) -> Self {
        let longest_line_end = self.starts_at + LONGEST_ESTIMATED_LINE;
        let line_window_end = next_line_start.map_or(longest_line_end, |next_start| {
            next_start.min(longest_line_end)
        });
        match self.word_timing {
            WordTiming::Sung => Self {
                ends_at: self
                    .word_starts
                    .last()
                    .map_or(line_window_end, |last_word| {
                        last_word.starts_at.max(line_window_end)
                    }),
                ..self
            },
            WordTiming::Estimated => Self {
                word_starts: estimated_word_starts(
                    &self.text,
                    self.starts_at,
                    line_window_end.saturating_sub(self.starts_at),
                ),
                ends_at: line_window_end,
                ..self
            },
        }
    }
}

fn estimated_word_starts(text: &str, line_start: Duration, line_span: Duration) -> Vec<WordStart> {
    let character_count = text.chars().count().max(1);
    iter::once(' ')
        .chain(text.chars())
        .zip(text.char_indices())
        .enumerate()
        .filter(|(_, (preceding_character, (_, word_character)))| {
            preceding_character.is_whitespace() && !word_character.is_whitespace()
        })
        .map(|(character_position, (_, (text_offset, _)))| WordStart {
            starts_at: line_start
                + line_span.mul_f64(character_position as f64 / character_count as f64),
            text_offset,
        })
        .collect()
}

struct ParsedWordTag<'a> {
    starts_at: Duration,
    after_tag: &'a str,
}

struct WordTaggedText {
    text: String,
    word_starts: Vec<WordStart>,
}

fn strip_word_tags(tagged_text: &str) -> WordTaggedText {
    let mut text = String::new();
    let mut word_starts = Vec::new();
    let mut remaining_text = tagged_text;
    while let Some((before_tag, after_open)) = remaining_text.split_once('<') {
        text.push_str(before_tag);
        let parsed_tag = after_open
            .split_once('>')
            .and_then(|(tag_body, after_tag)| {
                Some(ParsedWordTag {
                    starts_at: parse_lrc_timestamp(tag_body)?,
                    after_tag,
                })
            });
        if let Some(word_tag) = parsed_tag {
            word_starts.push(WordStart {
                starts_at: word_tag.starts_at,
                text_offset: text.len(),
            });
            remaining_text = word_tag.after_tag;
        } else {
            text.push('<');
            remaining_text = after_open;
        }
    }
    text.push_str(remaining_text);
    let leading_whitespace = text.len() - text.trim_start().len();
    let trimmed_text = text.trim().to_owned();
    WordTaggedText {
        word_starts: word_starts
            .into_iter()
            .map(|word_start| WordStart {
                text_offset: word_start
                    .text_offset
                    .saturating_sub(leading_whitespace)
                    .min(trimmed_text.len()),
                ..word_start
            })
            .collect(),
        text: trimmed_text,
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

    fn has_synced_lyrics(&self) -> bool {
        self.synced_lyrics
            .as_deref()
            .is_some_and(|lrc_text| !lrc_text.trim().is_empty())
    }
}

fn closest_search_match(candidate_records: Vec<LrclibRecord>, query: &LyricsQuery) -> TrackLyrics {
    candidate_records
        .into_iter()
        .filter(|record| record.duration_mismatch_secs(query) <= DURATION_TOLERANCE_SECS)
        .min_by(|left, right| {
            right
                .has_synced_lyrics()
                .cmp(&left.has_synced_lyrics())
                .then(
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
        match (synced_lines, plain_text) {
            (Some(timed_lines), _) => Self::Synced(timed_lines),
            (None, Some(plain_text)) => Self::PlainOnly(plain_text),
            (None, None) if record.instrumental => Self::Instrumental,
            (None, None) => Self::Missing,
        }
    }
}

fn parse_lrc_lines(lrc_text: &str) -> Vec<TimedLyricLine> {
    let mut timed_lines: Vec<_> = lrc_text.lines().flat_map(timed_lines_of_row).collect();
    timed_lines.sort_by_key(|timed_line| timed_line.starts_at);
    let next_line_starts: Vec<Option<Duration>> = timed_lines
        .iter()
        .skip(1)
        .map(|timed_line| Some(timed_line.starts_at))
        .chain(iter::once(None))
        .collect();
    timed_lines
        .into_iter()
        .zip(next_line_starts)
        .map(|(timed_line, next_line_start)| timed_line.with_completed_word_timing(next_line_start))
        .collect()
}

fn timed_lines_of_row(lrc_row: &str) -> Vec<TimedLyricLine> {
    let mut start_times = Vec::new();
    let mut remaining_row = lrc_row.trim();
    while let Some((tag_body, after_tag)) = remaining_row
        .strip_prefix('[')
        .and_then(|tagged_row| tagged_row.split_once(']'))
    {
        let Some(starts_at) = parse_lrc_timestamp(tag_body) else {
            return Vec::new();
        };
        start_times.push(starts_at);
        remaining_row = after_tag;
    }
    let word_tagged_text = strip_word_tags(remaining_row);
    let word_timing = if word_tagged_text.word_starts.is_empty() {
        WordTiming::Estimated
    } else {
        WordTiming::Sung
    };
    let first_line_start = start_times.first().copied().unwrap_or_default();
    start_times
        .into_iter()
        .map(|starts_at| TimedLyricLine {
            starts_at,
            ends_at: starts_at,
            text: word_tagged_text.text.clone(),
            word_starts: word_tagged_text
                .word_starts
                .iter()
                .map(|word_start| WordStart {
                    starts_at: starts_at + word_start.starts_at.saturating_sub(first_line_start),
                    ..*word_start
                })
                .collect(),
            word_timing,
        })
        .collect()
}

fn parse_lrc_timestamp(tag_body: &str) -> Option<Duration> {
    let (minutes_text, seconds_text) = tag_body.split_once(':')?;
    let minutes: u64 = minutes_text.trim().parse().ok()?;
    let seconds: f64 = seconds_text.trim().parse().ok()?;
    Duration::try_from_secs_f64(seconds)
        .ok()
        .map(|seconds_part| Duration::from_secs(minutes * 60) + seconds_part)
}

fn needs_search_fallback(fetch_error: &http_json_client::Error) -> bool {
    matches!(
        fetch_error,
        http_json_client::Error::Status {
            status: NOT_FOUND_STATUS,
            ..
        }
    ) || fetch_error.is_temporary()
}

async fn fetch_track_lyrics(query: &LyricsQuery) -> Result<TrackLyrics, http_json_client::Error> {
    let exact_lookup = http_json_client::get_json::<LrclibRecord>(
        &query.lookup_url(),
        &[("User-Agent", USER_AGENT)],
        FETCH_TIMEOUT_SECS,
    )
    .await;
    if exact_lookup.as_ref().is_err_and(needs_search_fallback) {
        return search_track_lyrics(query).await;
    }
    Ok(TrackLyrics::from(exact_lookup?))
}

async fn search_track_lyrics(query: &LyricsQuery) -> Result<TrackLyrics, http_json_client::Error> {
    let candidate_records = http_json_client::get_json::<Vec<LrclibRecord>>(
        &query.search_url(),
        &[("User-Agent", USER_AGENT)],
        FETCH_TIMEOUT_SECS,
    )
    .await?;
    Ok(closest_search_match(candidate_records, query))
}

impl SnapshotSource for TrackLyrics {
    type Error = http_json_client::Error;
    type StreamInputs = LyricsQuery;

    const NAME: &'static str = "On LRCLIB";

    fn delivery_stream(query: LyricsQuery) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        SnapshotDelivery::read_until_answered_deliveries(
            move || {
                let query = query.clone();
                async move { fetch_track_lyrics(&query).await }
            },
            RETRY_DELAYS,
            http_json_client::Error::is_temporary,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_texts(track_lyrics: &TrackLyrics) -> Vec<(f64, &str)> {
        let TrackLyrics::Synced(timed_lines) = track_lyrics else {
            return Vec::new();
        };
        timed_lines
            .iter()
            .map(|timed_line| (timed_line.starts_at.as_secs_f64(), timed_line.text.as_str()))
            .collect()
    }

    #[test]
    fn lrc_rows_skip_metadata_and_repeat_lines_with_several_timestamps() {
        let track_lyrics = TrackLyrics::synced_from_lrc(
            "[ar:The Harbour Lights]\n[ti:Low Tide]\n[00:12.50]Salt on the \
             window\n[00:20.00][01:05.25]Low tide, low tide\n[00:30.00]\n",
        );
        assert_eq!(
            line_texts(&track_lyrics),
            [
                (12.5, "Salt on the window"),
                (20.0, "Low tide, low tide"),
                (30.0, ""),
                (65.25, "Low tide, low tide"),
            ]
        );
    }

    #[test]
    fn current_line_is_the_last_one_that_has_started() {
        let track_lyrics = TrackLyrics::synced_from_lrc("[00:10.00]first\n[00:20.00]second");
        assert_eq!(
            track_lyrics.current_line_index(Duration::from_secs(5)),
            None
        );
        assert_eq!(
            track_lyrics.current_line_index(Duration::from_secs(10)),
            Some(0)
        );
        assert_eq!(
            track_lyrics.current_line_index(Duration::from_secs(25)),
            Some(1)
        );
    }

    #[test]
    fn record_prefers_synced_lyrics_and_falls_back_to_plain_then_instrumental() {
        let synced_record: LrclibRecord = serde_json::from_str(
            r#"{"id": 1, "trackName": "Low Tide", "instrumental": false,
                "plainLyrics": "Salt on the window", "syncedLyrics": "[00:12.50]Salt on the window"}"#,
        )
        .expect("the record parses");
        assert_eq!(
            line_texts(&TrackLyrics::from(synced_record)),
            [(12.5, "Salt on the window")]
        );
        let plain_record: LrclibRecord = serde_json::from_str(
            r#"{"instrumental": false, "plainLyrics": "Salt on the window", "syncedLyrics": null}"#,
        )
        .expect("the record parses");
        assert_eq!(
            TrackLyrics::from(plain_record),
            TrackLyrics::PlainOnly("Salt on the window".to_owned())
        );
        let instrumental_record: LrclibRecord = serde_json::from_str(
            r#"{"instrumental": true, "plainLyrics": null, "syncedLyrics": null}"#,
        )
        .expect("the record parses");
        assert_eq!(
            TrackLyrics::from(instrumental_record),
            TrackLyrics::Instrumental
        );
    }

    #[test]
    fn lookup_url_encodes_artist_title_and_duration() {
        let query = LyricsQuery {
            artist: "Felix Räuber".to_owned(),
            title: "Stay Here".to_owned(),
            album: String::new(),
            duration_secs: 241,
        };
        assert_eq!(
            query.lookup_url(),
            "https://lrclib.net/api/get?artist_name=Felix+R%C3%A4uber&track_name=Stay+Here&duration=241"
        );
        let query_with_album = LyricsQuery {
            album: "Grace".to_owned(),
            ..query
        };
        assert_eq!(
            query_with_album.lookup_url(),
            "https://lrclib.net/api/get?artist_name=Felix+R%C3%A4uber&track_name=Stay+Here&album_name=Grace&duration=241"
        );
    }

    #[test]
    fn search_fallback_picks_the_synced_version_closest_in_length() {
        let query = LyricsQuery {
            artist: "Jeff Buckley".to_owned(),
            title: "Mojo Pin".to_owned(),
            album: "Grace".to_owned(),
            duration_secs: 342,
        };
        let candidate_records: Vec<LrclibRecord> = serde_json::from_str(
            r#"[
                {"duration": 400.0, "syncedLyrics": "[00:01.00]live version"},
                {"duration": 343.0, "plainLyrics": "unsynced studio version", "syncedLyrics": null},
                {"duration": 341.0, "syncedLyrics": "[00:01.00]document version"},
                {"duration": 342.0, "syncedLyrics": "[00:01.00]grace version"}
            ]"#,
        )
        .expect("the search results parse");
        assert_eq!(
            line_texts(&closest_search_match(candidate_records, &query)),
            [(1.0, "grace version")]
        );
        assert_eq!(
            closest_search_match(Vec::new(), &query),
            TrackLyrics::Missing
        );
    }

    #[test]
    fn enhanced_lrc_word_tags_light_up_each_word_as_it_is_sung() {
        let track_lyrics = TrackLyrics::synced_from_lrc(
            "[00:10.00]<00:10.00>Low <00:10.80>tide, <00:11.50>low <00:12.40>tide",
        );
        let current_line = track_lyrics
            .current_line(Duration::from_secs(11))
            .expect("the line has started");
        assert_eq!(current_line.word_timing, WordTiming::Sung);
        assert_eq!(current_line.text, "Low tide, low tide");
        let swept_word_at = |seconds: f64| {
            let sweep = current_line.sweep(Duration::from_secs_f64(seconds));
            &current_line.text[sweep.segment_start..sweep.segment_end]
        };
        assert_eq!(swept_word_at(10.0), "Low ");
        assert_eq!(swept_word_at(11.0), "tide, ");
        assert_eq!(swept_word_at(13.0), "tide");
    }

    #[test]
    fn untagged_lines_spread_their_words_until_the_next_line() {
        let track_lyrics =
            TrackLyrics::synced_from_lrc("[00:10.00]one two three four\n[00:14.00]next");
        let current_line = track_lyrics
            .current_line(Duration::from_secs(10))
            .expect("the line has started");
        assert_eq!(current_line.word_timing, WordTiming::Estimated);
        let swept_word_at = |seconds: f64| {
            let sweep = current_line.sweep(Duration::from_secs_f64(seconds));
            &current_line.text[sweep.segment_start..sweep.segment_end]
        };
        assert_eq!(swept_word_at(10.0), "one ");
        assert_eq!(swept_word_at(11.5), "two ");
        assert_eq!(swept_word_at(13.9), "four");
        assert_eq!(current_line.ends_at, Duration::from_secs(14));
    }

    #[test]
    fn a_long_pause_before_the_next_line_does_not_slow_the_estimate() {
        let track_lyrics = TrackLyrics::synced_from_lrc("[00:00.00]one two\n[01:00.00]next");
        let current_line = track_lyrics
            .current_line(Duration::ZERO)
            .expect("the line has started");
        let finished_sweep = current_line.sweep(LONGEST_ESTIMATED_LINE);
        assert_eq!(finished_sweep.segment_end, current_line.text.len());
        assert!((finished_sweep.segment_fraction - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn the_sweep_moves_through_a_word_between_its_start_and_the_next() {
        let track_lyrics =
            TrackLyrics::synced_from_lrc("[00:10.00]<00:10.00>Low <00:12.00>tide\n[00:20.00]next");
        let current_line = track_lyrics
            .current_line(Duration::from_secs(11))
            .expect("the line has started");
        let halfway_sweep = current_line.sweep(Duration::from_secs(11));
        assert_eq!(
            (halfway_sweep.segment_start, halfway_sweep.segment_end),
            (0, 4)
        );
        assert!((halfway_sweep.segment_fraction - 0.5).abs() < f32::EPSILON);
    }
}
