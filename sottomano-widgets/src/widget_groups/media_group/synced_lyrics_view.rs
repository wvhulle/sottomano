use std::time::{Duration, Instant};

use iced::alignment::Horizontal;
use sottomano_config::Config;
use sottomano_sources::media_playback_audio::{
    lrclib_synced_lyrics::{TimedLyricLine, TrackLyrics},
    mpris_players::Media,
};

use super::{MediaMessage, MediaTarget, PlayerTarget};
use crate::{
    clock::Clock,
    widgets::{
        group_item::{GroupItem, ItemHover, ItemIcon, ItemIdentity, ItemValue},
        group_item_reading::GroupItemReading,
        icon_glyph::IconGlyph,
        karaoke_lyric_line::{
            INSTRUMENTAL_BREAK_MARK, KaraokeColors, KaraokeLyricLine, PlaybackPosition,
        },
        keyed_record_table::{ColumnHeading, TableRecord},
        latest_snapshot::LatestSnapshot,
        phosphor::{Phosphor, Weight},
        reading_display_state::{ReadingDisplayState, ReadingStateTint},
        tooltip_body::{Section, TooltipBody},
        value_cells::ValueCell,
    },
};

const LINES_BEFORE_CURRENT: usize = 2;
const LINES_AFTER_CURRENT: usize = 3;

impl GroupItemReading for TrackLyrics {
    fn name(&self) -> &'static str {
        "Lyrics"
    }

    fn icon(&self) -> IconGlyph {
        match self {
            TrackLyrics::Synced(_) => Phosphor::MicrophoneStage(Weight::Regular).into(),
            TrackLyrics::PlainOnly(_) | TrackLyrics::Instrumental | TrackLyrics::Missing => {
                Self::fallback_icon()
            }
        }
    }

    fn fallback_icon() -> IconGlyph {
        Phosphor::MicrophoneSlash(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match self {
            TrackLyrics::Synced(_) => ReadingDisplayState::Idle,
            TrackLyrics::PlainOnly(_) | TrackLyrics::Instrumental | TrackLyrics::Missing => {
                ReadingDisplayState::Off
            }
        }
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([lyrics_section(self, Duration::ZERO)])
    }
}

fn sung_text(timed_line: &TimedLyricLine) -> &str {
    if timed_line.text.is_empty() {
        INSTRUMENTAL_BREAK_MARK
    } else {
        &timed_line.text
    }
}

pub(super) fn lyrics_section(track_lyrics: &TrackLyrics, played: Duration) -> Section {
    let TrackLyrics::Synced(timed_lines) = track_lyrics else {
        let lookup_outcome = match track_lyrics {
            TrackLyrics::PlainOnly(_) => "LRCLIB has only unsynced lyrics for this track",
            TrackLyrics::Instrumental => "LRCLIB lists this track as instrumental",
            TrackLyrics::Missing | TrackLyrics::Synced(_) => "LRCLIB has no lyrics for this track",
        };
        return Section::note(lookup_outcome);
    };
    let current_index = track_lyrics.current_line_index(played);
    let upcoming_start = current_index.map_or(0, |index| index + 1);
    let past_start = current_index
        .unwrap_or_default()
        .saturating_sub(LINES_BEFORE_CURRENT);
    let lyric_record = |timed_line: &TimedLyricLine, lyric_cell: fn(String) -> ValueCell| {
        TableRecord::new(
            ValueCell::duration(timed_line.starts_at),
            [lyric_cell(sung_text(timed_line).to_owned())],
        )
    };
    let past_records: Vec<_> = timed_lines
        .iter()
        .take(current_index.unwrap_or_default())
        .skip(past_start)
        .map(|timed_line| lyric_record(timed_line, ValueCell::Hint))
        .collect();
    let current_records: Vec<_> = current_index
        .and_then(|index| timed_lines.get(index))
        .map(|timed_line| lyric_record(timed_line, ValueCell::Text))
        .into_iter()
        .collect();
    let upcoming_records: Vec<_> = timed_lines
        .iter()
        .skip(upcoming_start)
        .take(LINES_AFTER_CURRENT)
        .map(|timed_line| lyric_record(timed_line, ValueCell::Hint))
        .collect();
    Section::grouped_column_table(
        [ColumnHeading::untitled().align_x(Horizontal::Left)],
        [past_records, current_records, upcoming_records],
    )
    .titled("Lyrics")
}

fn singing_line<'a>(
    lyrics_snapshot: &'a LatestSnapshot<TrackLyrics>,
    media: &Media,
    sampled_at: Instant,
) -> Option<&'a TimedLyricLine> {
    let played = media.progress.played(media.status, sampled_at)?;
    lyrics_snapshot.get()?.current_line(played)
}

pub(super) fn lyrics_item<'a>(
    config: &'a Config,
    media: &'a Media,
    lyrics_snapshot: &'a LatestSnapshot<TrackLyrics>,
    sampled_at: Instant,
) -> Option<GroupItem<'a, MediaMessage>> {
    let Some(timed_line) = singing_line(lyrics_snapshot, media, sampled_at) else {
        return lyrics_snapshot.group_item(PlayerTarget::of(&media.bus, MediaTarget::Lyrics));
    };
    Some(current_lyric_line(config, media, timed_line, sampled_at))
}

fn current_lyric_line<'a>(
    config: &'a Config,
    media: &'a Media,
    timed_line: &'a TimedLyricLine,
    sampled_at: Instant,
) -> GroupItem<'a, MediaMessage> {
    let identity = ItemIdentity {
        name: "Lyrics".into(),
        icon: ItemIcon::IconGlyph {
            glyph: Phosphor::MicrophoneStage(Weight::Regular).into(),
            state: ReadingDisplayState::Idle,
        },
    };
    let hover = ItemHover::WholeItem(PlayerTarget::of(&media.bus, MediaTarget::Lyrics).into());
    let value = ItemValue::drawn(move |reveal| {
        let tinted = move |state| ReadingStateTint {
            state,
            opacity: reveal,
        };
        KaraokeLyricLine::new(
            timed_line,
            PlaybackPosition {
                progress: media.progress,
                status: media.status,
                sampled_at,
            },
            &config.theme.font,
            KaraokeColors {
                sung: Box::new(move |theme| tinted(ReadingDisplayState::Live).color(theme)),
                unsung: Box::new(move |theme| tinted(ReadingDisplayState::Off).color(theme)),
            },
        )
        .into()
    });
    GroupItem::composed(identity, hover, value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::{keyed_record_table::KeyedTableRow, tooltip_body::SectionBody};

    fn verse_lines() -> TrackLyrics {
        TrackLyrics::synced_from_lrc(
            &(0..10)
                .map(|verse_index| format!("[00:{:02}.00]verse {verse_index}", verse_index * 5))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }

    fn table_rows(section: &Section) -> Vec<&'static str> {
        let SectionBody::ColumnTable(table) = section.body() else {
            panic!("synced lyrics render as a table");
        };
        table
            .rows()
            .iter()
            .map(|table_row| match table_row {
                KeyedTableRow::GroupSeparator => "separator",
                _ => "line",
            })
            .collect()
    }

    #[test]
    fn past_current_and_upcoming_lines_are_separated() {
        let section = lyrics_section(&verse_lines(), Duration::from_secs(23));
        assert_eq!(
            table_rows(&section),
            [
                "line",
                "line",
                "separator",
                "line",
                "separator",
                "line",
                "line",
                "line"
            ]
        );
    }

    #[test]
    fn before_the_first_line_only_upcoming_lines_show() {
        let track_lyrics = TrackLyrics::synced_from_lrc("[00:10.00]first\n[00:20.00]second");
        let section = lyrics_section(&track_lyrics, Duration::from_secs(1));
        assert_eq!(table_rows(&section), ["line", "line"]);
    }
}
