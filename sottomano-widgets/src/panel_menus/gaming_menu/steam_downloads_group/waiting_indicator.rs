use chrono::{Locale, TimeDelta};
use sottomano_desktop_sources::steam_client_files::steam_content_activity::{
    QueuedUpdateHold, SteamContentEntry,
};

use super::opens_downloads;
use crate::{
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator, indicator_drawing::ReadoutLabel,
        indicator_state::IndicatorState,
    },
    widgets::localized_durations::duration_text,
};

pub(super) fn waiting_indicator(
    entry: &SteamContentEntry,
    clock: Clock,
    locale: Locale,
) -> GroupIndicator<'_> {
    let (installed_app, glyph, label) = match entry {
        SteamContentEntry::Queued {
            installed_app,
            hold: QueuedUpdateHold::Waiting,
        } => (
            installed_app,
            Phosphor::Queue(Weight::Regular),
            "queued".to_owned(),
        ),
        SteamContentEntry::Queued {
            installed_app,
            hold: QueuedUpdateHold::Suspended,
        } => (
            installed_app,
            Phosphor::Pause(Weight::Regular),
            "paused".to_owned(),
        ),
        SteamContentEntry::Delayed {
            installed_app,
            update_due_at,
        } => {
            let remaining: TimeDelta = *update_due_at - clock.local().naive_local();
            let label = if remaining > TimeDelta::zero() {
                format!("update in {}", duration_text(remaining, locale))
            } else {
                "update due".to_owned()
            };
            (
                installed_app,
                Phosphor::ClockCountdown(Weight::Regular),
                label,
            )
        }
        SteamContentEntry::Downloading(installed_app) => (
            installed_app,
            Phosphor::DownloadSimple(Weight::Regular),
            "downloading".to_owned(),
        ),
    };
    opens_downloads(
        GroupIndicator::glyph_only(
            installed_app.name.clone(),
            glyph.into(),
            IndicatorState::Idle,
        )
        .with_label(ReadoutLabel::Labeled(label)),
    )
}

pub(super) fn hidden_entries_indicator<'activity>(
    hidden_count: usize,
) -> GroupIndicator<'activity> {
    opens_downloads(GroupIndicator::glyph_only(
        format!("{hidden_count} more"),
        Phosphor::DotsThree(Weight::Regular).into(),
        IndicatorState::Idle,
    ))
}
