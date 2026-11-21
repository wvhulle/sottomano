use std::iter;

use bytesize::ByteSize;
use chrono::{Locale, TimeDelta};
use sottomano_desktop_sinks::steam_client_pages::SteamClientPage;
use sottomano_desktop_sources::steam_client_files::{
    steam_app_manifests::InstalledSteamApp,
    steam_content_activity::{QueuedUpdateHold, SteamContentActivity, SteamContentEntry},
};
use uom::si::{f64::Ratio, ratio::ratio};

use super::SteamClientInteraction;
use crate::{
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    indicator_groups::{
        indicator_group_content::{IndicatorGroupContent, IndicatorGroupExpansion},
        indicator_group_identity::IndicatorGroupFamily,
    },
    widgets::{
        localized_durations::duration_text, localized_quantities::Quantity,
        value_cells::LabeledValue,
    },
};

const SHOWN_WAITING_ENTRIES: usize = 4;
const OPEN_DOWNLOADS: &str = "open Steam's downloads page";

pub(super) fn is_downloading(entry: &SteamContentEntry) -> bool {
    matches!(entry, SteamContentEntry::Downloading(_))
}

fn downloaded_share(installed_app: &InstalledSteamApp) -> Ratio {
    let downloaded = installed_app.bytes_downloaded as f64;
    let total = installed_app.bytes_to_download.max(1) as f64;
    Ratio::new::<ratio>((downloaded / total).clamp(0.0, 1.0))
}

fn opens_downloads(indicator: GroupIndicator<'_>) -> GroupIndicator<'_> {
    indicator.on_press(
        OPEN_DOWNLOADS,
        SteamClientInteraction::OpenPage(SteamClientPage::Downloads),
    )
}

fn downloading_indicator(installed_app: &InstalledSteamApp, locale: Locale) -> GroupIndicator<'_> {
    let share = downloaded_share(installed_app);
    let percent = (share.get::<ratio>() * 100.0).round();
    opens_downloads(
        GroupIndicator::glyph_only(
            installed_app.name.clone(),
            Phosphor::DownloadSimple(Weight::Regular).into(),
            IndicatorState::On,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(format!("{percent}%")),
            gauge: ReadoutGauge::Gauged {
                fill: share,
                live: share,
            },
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| {
            vec![Section::new(
                "Downloading",
                dictionary_table([
                    LabeledValue::new(
                        "Downloaded",
                        Quantity::size(ByteSize::b(installed_app.bytes_downloaded)),
                    ),
                    LabeledValue::new(
                        "To download",
                        Quantity::size(ByteSize::b(installed_app.bytes_to_download)),
                    ),
                    LabeledValue::new("Progress", Quantity::share(share).localized_text(locale)),
                ]),
            )]
        })),
    )
}

fn waiting_indicator(
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

fn hidden_entries_indicator<'activity>(hidden_count: usize) -> GroupIndicator<'activity> {
    opens_downloads(GroupIndicator::glyph_only(
        format!("{hidden_count} more"),
        Phosphor::DotsThree(Weight::Regular).into(),
        IndicatorState::Idle,
    ))
}

fn rate_indicator(activity: &SteamContentActivity) -> GroupIndicator<'_> {
    let rate = format!("{:.1} Mbit/s", activity.download_rate_megabits_per_second);
    opens_downloads(
        GroupIndicator::glyph_only(
            "Download rate",
            Phosphor::DownloadSimple(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_label(ReadoutLabel::Labeled(rate)),
    )
}

pub(super) fn downloads_group<'activity>(
    activity: &'activity SteamContentActivity,
    expansion: &'activity IndicatorGroupExpansion,
    clock: Clock,
    locale: Locale,
) -> Option<IndicatorGroupContent<'activity>> {
    if activity.entries.is_empty() {
        return None;
    }
    let downloading = activity.entries.iter().filter_map(|entry| match entry {
        SteamContentEntry::Downloading(installed_app) => {
            Some(downloading_indicator(installed_app, locale))
        }
        SteamContentEntry::Queued { .. } | SteamContentEntry::Delayed { .. } => None,
    });
    let waiting_entries: Vec<&SteamContentEntry> = activity
        .entries
        .iter()
        .filter(|entry| !is_downloading(entry))
        .collect();
    let (shown_waiting, hidden_waiting) =
        waiting_entries.split_at(waiting_entries.len().min(SHOWN_WAITING_ENTRIES));
    let waiting = shown_waiting
        .iter()
        .map(|entry| waiting_indicator(entry, clock, locale));
    let mut rows: Vec<GroupIndicator<'activity>> = downloading.chain(waiting).collect();
    if !hidden_waiting.is_empty() {
        rows.push(hidden_entries_indicator(hidden_waiting.len()));
    }
    let headline_indicator = rate_indicator(activity)
        .with_headline_glyph(Phosphor::DownloadSimple(Weight::Regular).into());
    Some(IndicatorGroupContent::expandable(
        IndicatorGroupFamily::SteamDownloads,
        headline_indicator,
        expansion,
        SteamClientInteraction::DownloadsExpansion,
        iter::once(rate_indicator(activity)).chain(rows),
    ))
}
