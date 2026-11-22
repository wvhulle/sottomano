use std::{iter, time::Duration};

use super::{LONGEST_ESTIMATED_LINE, TimedLyricLine, WordStart, WordTiming};

impl TimedLyricLine {
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
                    line_window_end
                        .checked_sub(self.starts_at)
                        .expect("sorted lines end their window at or after their start"),
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

struct ParsedWordTag<'line> {
    starts_at: Duration,
    after_tag: &'line str,
}

struct WordTaggedText {
    text: String,
    word_starts: Vec<WordStart>,
}

#[expect(
    clippy::disallowed_methods,
    reason = "a word tag placed before the leading whitespace starts at the beginning of the \
              trimmed text"
)]
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

pub(super) fn parse_lrc_lines(lrc_text: &str) -> Vec<TimedLyricLine> {
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

#[expect(
    clippy::disallowed_methods,
    reason = "a word tag timed before its line comes from a malformed lyrics file and starts with \
              the line"
)]
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
    let (minutes, seconds) = match (
        minutes_text.trim().parse::<u64>(),
        seconds_text.trim().parse::<f64>(),
    ) {
        (Ok(minutes), Ok(seconds)) => (minutes, seconds),
        (Err(parse_failure), _) => {
            tracing::debug!("lyrics: tag {tag_body:?} has no timestamp minutes: {parse_failure}");
            return None;
        }
        (_, Err(parse_failure)) => {
            tracing::debug!("lyrics: tag {tag_body:?} has no timestamp seconds: {parse_failure}");
            return None;
        }
    };
    Duration::try_from_secs_f64(seconds)
        .inspect_err(|range_failure| {
            tracing::warn!("lyrics: timestamp tag {tag_body:?} is out of range: {range_failure}");
        })
        .ok()
        .map(|seconds_part| Duration::from_secs(minutes * 60) + seconds_part)
}
