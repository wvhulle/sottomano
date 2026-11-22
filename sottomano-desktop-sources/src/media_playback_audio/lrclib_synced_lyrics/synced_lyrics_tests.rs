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
    let track_lyrics = TrackLyrics::synced_from_lrc("[00:10.00]one two three four\n[00:14.00]next");
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
