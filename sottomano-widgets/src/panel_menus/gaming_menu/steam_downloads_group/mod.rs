mod download_rate_indicator;
mod downloading_indicator;
mod waiting_indicator;

use chrono::Locale;
use iced::{Subscription, Task};
use sottomano_desktop_sinks::steam_client_pages::{SteamClientPage, open_steam_client_page};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    steam_client_files::steam_content_activity::{SteamContentActivity, SteamContentEntry},
};

use self::{
    download_rate_indicator::{download_queue_indicator, download_rate_indicator},
    downloading_indicator::downloading_indicator,
    waiting_indicator::{hidden_entries_indicator, waiting_indicator},
};
use crate::{
    background_work::{logged_failure_task, snapshot_subscription},
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
            IndicatorListing,
        },
        indicator_group_identity::IndicatorGroupFamily,
    },
    widgets::subscribed_snapshot::SubscribedSnapshot,
};

const SHOWN_WAITING_ENTRIES: usize = 4;
const OPEN_DOWNLOADS: &str = "open Steam's downloads page";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SteamDownloadActivity {
    Downloading,
    Quiet,
}

#[derive(Debug)]
pub enum SteamDownloadsMessage {
    ContentActivity(SnapshotDelivery<SteamContentActivity>),
}

#[derive(Debug, Clone, Copy)]
pub enum SteamDownloadsInteraction {
    OpenDownloadsPage,
    Expansion(IndicatorGroupExpansionMessage),
}

#[derive(Default)]
pub struct SteamDownloadsGroup {
    content_activity: SubscribedSnapshot<SteamContentActivity>,
    expansion: IndicatorGroupExpansion,
}

impl From<&SteamContentActivity> for SteamDownloadActivity {
    fn from(activity: &SteamContentActivity) -> Self {
        if activity
            .entries
            .iter()
            .any(|entry| matches!(entry, SteamContentEntry::Downloading(_)))
        {
            SteamDownloadActivity::Downloading
        } else {
            SteamDownloadActivity::Quiet
        }
    }
}

fn opens_downloads(indicator: GroupIndicator<'_>) -> GroupIndicator<'_> {
    indicator.on_press(OPEN_DOWNLOADS, SteamDownloadsInteraction::OpenDownloadsPage)
}

impl SteamDownloadsGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription() -> Subscription<SteamDownloadsMessage> {
        snapshot_subscription::<SteamContentActivity>(())
            .map(SteamDownloadsMessage::ContentActivity)
    }

    pub fn update(
        &mut self,
        message: SteamDownloadsMessage,
        clock: Clock,
    ) -> Task<SteamDownloadsMessage> {
        match message {
            SteamDownloadsMessage::ContentActivity(delivery) => {
                self.content_activity.receive(delivery, clock);
            }
        }
        Task::none()
    }

    pub fn interact(
        &mut self,
        interaction: SteamDownloadsInteraction,
    ) -> Task<SteamDownloadsMessage> {
        match interaction {
            SteamDownloadsInteraction::OpenDownloadsPage => logged_failure_task(
                "open Steam downloads page".to_owned(),
                open_steam_client_page(SteamClientPage::Downloads),
            ),
            SteamDownloadsInteraction::Expansion(expansion) => {
                self.expansion.update(expansion);
                Task::none()
            }
        }
    }

    #[must_use]
    pub fn download_activity(&self) -> SteamDownloadActivity {
        self.content_activity
            .get()
            .map_or(SteamDownloadActivity::Quiet, SteamDownloadActivity::from)
    }

    #[must_use]
    pub fn group_contents(
        &self,
        clock: Clock,
        locale: Locale,
    ) -> Option<IndicatorGroupContent<'_>> {
        let activity = self.content_activity.get()?;
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
            .filter(|entry| !matches!(entry, SteamContentEntry::Downloading(_)))
            .collect();
        let (shown_waiting, hidden_waiting) =
            waiting_entries.split_at(waiting_entries.len().min(SHOWN_WAITING_ENTRIES));
        let waiting = shown_waiting
            .iter()
            .map(|entry| waiting_indicator(entry, clock, locale));
        let mut rows: Vec<GroupIndicator<'_>> = downloading.chain(waiting).collect();
        if !hidden_waiting.is_empty() {
            rows.push(hidden_entries_indicator(hidden_waiting.len()));
        }
        let (headline_indicator, rate_rows) = match SteamDownloadActivity::from(activity) {
            SteamDownloadActivity::Downloading => (
                download_rate_indicator(activity, locale),
                vec![download_rate_indicator(activity, locale)],
            ),
            SteamDownloadActivity::Quiet => (download_queue_indicator(), Vec::new()),
        };
        let headline_indicator = headline_indicator
            .with_headline_glyph(Phosphor::DownloadSimple(Weight::Regular).into());
        Some(IndicatorGroupContent::expandable(
            IndicatorGroupFamily::SteamDownloads,
            headline_indicator,
            &self.expansion,
            SteamDownloadsInteraction::Expansion,
            IndicatorListing::Listed(rate_rows.into_iter().chain(rows).collect()),
        ))
    }
}
