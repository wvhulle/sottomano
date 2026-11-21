use std::{collections::BTreeMap, future::ready, path::PathBuf};

use chrono::{NaiveDateTime, TimeDelta};
use futures::{Stream, StreamExt, stream};

use super::{
    appended_log_follow::appended_log_lines,
    steam_app_manifests::{InstalledSteamApp, installed_app_in_libraries, library_folder_paths},
    steam_log_lines::{
        LogLineShape, SteamAppId,
        content_log_lines::{ContentLogLine, ContentStateFlag, UpdateHold},
    },
    steam_root_location::SteamRoot,
};
use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::{ChangeWatchFailure, SnapshotDeliveryStreams},
    system_protocol_clients::inotify_directory_changes::{self, WatchTarget},
};

const CONTENT_LOG_FILE: &str = "content_log.txt";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueuedUpdateHold {
    Waiting,
    Suspended,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SteamContentEntry {
    Downloading(InstalledSteamApp),
    Queued {
        installed_app: InstalledSteamApp,
        hold: QueuedUpdateHold,
    },
    Delayed {
        installed_app: InstalledSteamApp,
        update_due_at: NaiveDateTime,
    },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SteamContentActivity {
    pub download_rate_megabits_per_second: f64,
    pub entries: Vec<SteamContentEntry>,
}

#[derive(Debug, Clone, PartialEq)]
struct LoggedAppState {
    flags: Vec<ContentStateFlag>,
    hold: UpdateHold,
    logged_at: NaiveDateTime,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ContentLogState {
    app_states: BTreeMap<SteamAppId, LoggedAppState>,
    download_rate_megabits_per_second: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContentStage {
    Downloading,
    Queued(QueuedUpdateHold),
    Delayed(NaiveDateTime),
    Settled,
}

impl LoggedAppState {
    fn stage(&self) -> ContentStage {
        let flagged = |flag: &ContentStateFlag| self.flags.contains(flag);
        match self.hold {
            UpdateHold::DelayedFor(delay) if flagged(&ContentStateFlag::UpdateRequired) => {
                match TimeDelta::from_std(delay) {
                    Ok(delay) => ContentStage::Delayed(self.logged_at + delay),
                    Err(out_of_range) => {
                        tracing::debug!("Steam update delay {delay:?} unusable: {out_of_range}");
                        ContentStage::Settled
                    }
                }
            }
            UpdateHold::Suspended if flagged(&ContentStateFlag::UpdateQueued) => {
                ContentStage::Queued(QueuedUpdateHold::Suspended)
            }
            _ if flagged(&ContentStateFlag::UpdateRunning) => ContentStage::Downloading,
            _ if flagged(&ContentStateFlag::UpdateQueued) => {
                ContentStage::Queued(QueuedUpdateHold::Waiting)
            }
            _ => ContentStage::Settled,
        }
    }
}

impl ContentLogState {
    pub fn absorb(&mut self, logged_at: NaiveDateTime, line: ContentLogLine) {
        match line {
            ContentLogLine::StateChanged {
                app_id,
                flags,
                hold,
            } => {
                self.app_states.insert(
                    app_id,
                    LoggedAppState {
                        flags,
                        hold,
                        logged_at,
                    },
                );
            }
            ContentLogLine::DownloadRate {
                megabits_per_second,
            } => self.download_rate_megabits_per_second = megabits_per_second,
            ContentLogLine::ClientStarted => *self = ContentLogState::default(),
            ContentLogLine::UpdateStarted { .. }
            | ContentLogLine::UpdateFinished { .. }
            | ContentLogLine::UpdateCanceled { .. }
            | ContentLogLine::Unrecognised => {}
        }
    }

    fn absorb_lines(&mut self, lines: &[String]) {
        for line in lines {
            if let LogLineShape::Stamped { logged_at, message } = LogLineShape::from(line.as_str())
            {
                self.absorb(logged_at, ContentLogLine::from(message));
            }
        }
    }

    fn activity(&self, library_folders: &[PathBuf]) -> SteamContentActivity {
        let mut entries: Vec<SteamContentEntry> = self
            .app_states
            .iter()
            .filter_map(|(app_id, app_state)| {
                let installed_app = || installed_app_in_libraries(library_folders, *app_id);
                match app_state.stage() {
                    ContentStage::Settled => None,
                    ContentStage::Downloading => {
                        installed_app().map(SteamContentEntry::Downloading)
                    }
                    ContentStage::Queued(hold) => {
                        installed_app().map(|installed_app| SteamContentEntry::Queued {
                            installed_app,
                            hold,
                        })
                    }
                    ContentStage::Delayed(update_due_at) => {
                        installed_app().map(|installed_app| SteamContentEntry::Delayed {
                            installed_app,
                            update_due_at,
                        })
                    }
                }
            })
            .collect();
        entries.sort_by_key(|entry| match entry {
            SteamContentEntry::Downloading(_) => 0,
            SteamContentEntry::Queued { .. } => 1,
            SteamContentEntry::Delayed { .. } => 2,
        });
        SteamContentActivity {
            download_rate_megabits_per_second: self.download_rate_megabits_per_second,
            entries,
        }
    }
}

enum ContentChange {
    LogLines(Vec<String>),
    ManifestsWritten,
}

impl SnapshotSource for SteamContentActivity {
    type Error = ChangeWatchFailure;
    type StreamInputs = ();

    const NAME: &'static str = "Steam downloads";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let followed = SteamRoot::located().map(|steam_root| {
            let library_folders =
                library_folder_paths(&steam_root).unwrap_or_else(|manifest_failure| {
                    tracing::debug!("{manifest_failure}");
                    Vec::new()
                });
            let manifest_watches = library_folders
                .iter()
                .map(|library_folder| WatchTarget::EntriesOf(library_folder.join("steamapps")))
                .collect();
            let log_changes = appended_log_lines(steam_root.log_file(CONTENT_LOG_FILE))
                .map(|appended| appended.map(ContentChange::LogLines));
            let manifest_changes = inotify_directory_changes::changes(manifest_watches)
                .map(|written| written.map(|()| ContentChange::ManifestsWritten));
            stream::select(log_changes, manifest_changes).scan(
                ContentLogState::default(),
                move |log_state, change| {
                    ready(Some(change.map(|change| {
                        if let ContentChange::LogLines(lines) = change {
                            log_state.absorb_lines(&lines);
                        }
                        log_state.activity(&library_folders)
                    })))
                },
            )
        });
        stream::iter(followed)
            .flatten()
            .map(SnapshotDelivery::live_push)
            .repeats_as_unchanged()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log_state_after(messages: &[&str]) -> ContentLogState {
        let mut log_state = ContentLogState::default();
        log_state.absorb_lines(
            &messages
                .iter()
                .map(|message| format!("[2026-11-20 11:14:45] {message}"))
                .collect::<Vec<_>>(),
        );
        log_state
    }

    fn app_content_stage(log_state: &ContentLogState, app_number: u32) -> ContentStage {
        log_state.app_states[&SteamAppId { number: app_number }].stage()
    }

    #[test]
    fn the_latest_state_line_decides_the_stage() {
        let log_state = log_state_after(&[
            "AppID 275850 state changed : Fully Installed,Update Queued,",
            "AppID 284160 state changed : Fully Installed,Update Queued,Update Running,",
            "AppID 228980 state changed : Update Required,Fully Installed,Update Queued,Update \
             Started, (Suspended)",
            "AppID 294100 state changed : Fully Installed,",
        ]);
        assert_eq!(
            app_content_stage(&log_state, 275_850),
            ContentStage::Queued(QueuedUpdateHold::Waiting)
        );
        assert_eq!(
            app_content_stage(&log_state, 284_160),
            ContentStage::Downloading
        );
        assert_eq!(
            app_content_stage(&log_state, 228_980),
            ContentStage::Queued(QueuedUpdateHold::Suspended)
        );
        assert_eq!(
            app_content_stage(&log_state, 294_100),
            ContentStage::Settled
        );
    }

    #[test]
    fn a_delayed_update_is_due_after_its_delay() {
        let log_state = log_state_after(&["AppID 1478970 state changed : Update Required,Fully \
                                           Installed, (Update delayed for 3600 secs)"]);
        let ContentStage::Delayed(update_due_at) = app_content_stage(&log_state, 1_478_970) else {
            panic!("expected a delayed update");
        };
        assert_eq!(update_due_at.format("%H:%M").to_string(), "12:14");
    }

    #[test]
    fn the_latest_rate_is_kept_and_a_restart_clears_everything() {
        let log_state = log_state_after(&["Current download rate: 12.500 Mbps"]);
        assert!((log_state.download_rate_megabits_per_second - 12.5).abs() < f64::EPSILON);
        let restarted = log_state_after(&[
            "AppID 275850 state changed : Fully Installed,Update Queued,",
            "Client version: 1794536281",
        ]);
        assert!(restarted.app_states.is_empty());
    }
}
