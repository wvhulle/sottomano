use std::time::{Duration, Instant};

use corredo::changing_text::KaraokeColors;
use iced::alignment::Horizontal;
use sottomano_desktop_sources::{
    SnapshotSource,
    media_playback_audio::{
        lrclib_synced_lyrics::{TimedLyricLine, TrackLyrics},
        mpris_players::Media,
    },
};
use sottomano_toml_config::Config;

use super::{MediaInteraction, MediaTarget, PlayerTarget};
use crate::{
    clock::Clock,
    group_items::{
        group_item::{GroupItem, GroupItemValue},
        reading_display_state::{ReadingDisplayState, ReadingStateTint},
        reading_row::ReadingRow,
        source_reading::GroupItemIcon,
    },
    hover_cards::hover_card_body::{Section, SectionTable, SectionWidget, headed_grouped_table},
    widget_groups::media_group::karaoke_lyric_line::{
        INSTRUMENTAL_BREAK_MARK, KaraokeLyricLine, PlaybackPosition,
    },
    widgets::{
        icon_glyph::IconGlyph,
        keyed_record_table::{ColumnHeading, TableRecord},
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
        value_cells::ValueCell,
    },
};

const LINES_BEFORE_CURRENT: usize = 2;
const LINES_AFTER_CURRENT: usize = 3;

pub(crate) const TRACK_LYRICS_FALLBACK_GLYPH: IconGlyph =
    IconGlyph::Phosphor(Phosphor::MicrophoneSlash(Weight::Regular));

fn track_lyrics_reading_row(track_lyrics: &TrackLyrics) -> ReadingRow<'_> {
    let (icon, state) = match track_lyrics {
        TrackLyrics::Synced(_) => (
            Phosphor::MicrophoneStage(Weight::Regular).into(),
            ReadingDisplayState::Idle,
        ),
        TrackLyrics::PlainOnly(_) | TrackLyrics::Instrumental | TrackLyrics::Missing => {
            (TRACK_LYRICS_FALLBACK_GLYPH, ReadingDisplayState::Off)
        }
    };
    ReadingRow::glyph_only(
        TrackLyrics::NAME,
        icon,
        state,
        Box::new(move |_| vec![lyrics_section(track_lyrics, Duration::ZERO)]),
    )
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
        return Section::new("Lookup", SectionWidget::paragraph(lookup_outcome));
    };
    Section::new(
        "Lines",
        synced_lyrics_table(track_lyrics, timed_lines, played),
    )
}

fn synced_lyrics_table(
    track_lyrics: &TrackLyrics,
    timed_lines: &[TimedLyricLine],
    played: Duration,
) -> SectionTable {
    let current_index = track_lyrics.current_line_index(played);
    let upcoming_start = current_index.map_or(0, |index| index + 1);
    #[expect(
        clippy::disallowed_methods,
        reason = "near the first line fewer than LINES_BEFORE_CURRENT earlier lines exist, so the \
                  window starts at the first line"
    )]
    let past_start = current_index
        .unwrap_or_default()
        .saturating_sub(LINES_BEFORE_CURRENT);
    let lyric_record = |timed_line: &TimedLyricLine, lyric_cell: fn(String) -> ValueCell| {
        TableRecord::with_cells(
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
    headed_grouped_table(
        [ColumnHeading::untitled().align_x(Horizontal::Left)],
        [past_records, current_records, upcoming_records],
    )
}

fn singing_line<'a>(
    lyrics_snapshot: &'a SubscribedSnapshot<TrackLyrics>,
    media: &Media,
    sampled_at: Instant,
) -> Option<&'a TimedLyricLine> {
    let played = media.progress.played(media.status, sampled_at)?;
    lyrics_snapshot.get()?.current_line(played)
}

pub(super) fn lyrics_item<'a>(
    config: &'a Config,
    media: &'a Media,
    lyrics_snapshot: &'a SubscribedSnapshot<TrackLyrics>,
    sampled_at: Instant,
    clock: Clock,
) -> Option<GroupItem<'a, MediaInteraction>> {
    let Some(timed_line) = singing_line(lyrics_snapshot, media, sampled_at) else {
        return lyrics_snapshot.group_item(
            TRACK_LYRICS_FALLBACK_GLYPH,
            track_lyrics_reading_row,
            PlayerTarget {
                bus: media.bus.clone().into(),
                control: MediaTarget::Lyrics,
            },
            clock,
        );
    };
    Some(current_lyric_line(config, media, timed_line, sampled_at))
}

fn current_lyric_line<'a>(
    config: &'a Config,
    media: &'a Media,
    timed_line: &'a TimedLyricLine,
    sampled_at: Instant,
) -> GroupItem<'a, MediaInteraction> {
    let icon = GroupItemIcon::IconGlyph {
        glyph: Phosphor::MicrophoneStage(Weight::Regular).into(),
        state: ReadingDisplayState::Idle,
    };
    let value = GroupItemValue::drawn(move |reveal| {
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
    GroupItem::composed_pinnable(
        "Lyrics",
        icon,
        PlayerTarget {
            bus: media.bus.clone().into(),
            control: MediaTarget::Lyrics,
        },
        value,
        Box::new(|_| Vec::new()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::keyed_record_table::KeyedTableRow;

    fn verse_lines() -> TrackLyrics {
        TrackLyrics::synced_from_lrc(
            &(0..10)
                .map(|verse_index| format!("[00:{:02}.00]verse {verse_index}", verse_index * 5))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }

    fn table_rows(track_lyrics: &TrackLyrics, played: Duration) -> Vec<&'static str> {
        let TrackLyrics::Synced(timed_lines) = track_lyrics else {
            panic!("test lyrics are synced");
        };
        synced_lyrics_table(track_lyrics, timed_lines, played)
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
        assert_eq!(
            table_rows(&verse_lines(), Duration::from_secs(23)),
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
        assert_eq!(
            table_rows(&track_lyrics, Duration::from_secs(1)),
            ["line", "line"]
        );
    }
}
