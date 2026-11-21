use std::{
    collections::{BTreeMap, BTreeSet},
    future::ready,
    path::PathBuf,
};

use chrono::NaiveDateTime;
use futures::{Stream, StreamExt, stream};

use super::{
    appended_log_follow::appended_log_lines,
    steam_app_manifests::{InstalledSteamApp, installed_app_in_libraries, library_folder_paths},
    steam_log_lines::{LogLineShape, SteamAppId, game_process_log_lines::GameProcessLogLine},
    steam_root_location::{SteamClientPresence, SteamRoot},
};
use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::{ChangeWatchFailure, SnapshotDeliveryStreams},
    system_protocol_clients::sysfs_attribute_reads,
};

const GAME_PROCESS_LOG_FILE: &str = "gameprocess_log.txt";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningSteamGame {
    pub app_id: SteamAppId,
    pub launched_at: NaiveDateTime,
    pub tracked_processes: Vec<TrackedGameProcess>,
    pub installed_app: InstalledSteamApp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedGameProcess {
    pub process_id: u32,
    pub command_name: String,
}

fn live_tracked_processes(process_ids: &BTreeSet<u32>) -> Vec<TrackedGameProcess> {
    process_ids
        .iter()
        .filter_map(|process_id| {
            Some(TrackedGameProcess {
                process_id: *process_id,
                command_name: sysfs_attribute_reads::read_trimmed(format!(
                    "/proc/{process_id}/comm"
                ))?,
            })
        })
        .collect()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunningSteamGames {
    pub games: Vec<RunningSteamGame>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TrackedApp {
    launched_at: NaiveDateTime,
    tracked_process_ids: BTreeSet<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameProcessLogState {
    tracked_apps: BTreeMap<SteamAppId, TrackedApp>,
}

impl GameProcessLogState {
    pub fn absorb(&mut self, logged_at: NaiveDateTime, line: GameProcessLogLine) {
        match line {
            GameProcessLogLine::TrackingStarted { app_id, process_id } => {
                self.tracked_apps
                    .entry(app_id)
                    .or_insert_with(|| TrackedApp {
                        launched_at: logged_at,
                        tracked_process_ids: BTreeSet::new(),
                    })
                    .tracked_process_ids
                    .insert(process_id);
            }
            GameProcessLogLine::TrackingStopped { app_id, process_id } => {
                if let Some(tracked_app) = self.tracked_apps.get_mut(&app_id) {
                    tracked_app.tracked_process_ids.remove(&process_id);
                }
            }
            GameProcessLogLine::RemovedFromRunning { app_id } => {
                self.tracked_apps.remove(&app_id);
            }
            GameProcessLogLine::ClientStarted => self.tracked_apps.clear(),
            GameProcessLogLine::Unrecognised => {}
        }
    }

    fn absorb_lines(&mut self, lines: &[String]) {
        for line in lines {
            if let LogLineShape::Stamped { logged_at, message } = LogLineShape::from(line.as_str())
            {
                self.absorb(logged_at, GameProcessLogLine::from(message));
            }
        }
    }

    fn running_games(&self, library_folders: &[PathBuf]) -> RunningSteamGames {
        if SteamClientPresence::current() == SteamClientPresence::Stopped {
            return RunningSteamGames::default();
        }
        RunningSteamGames {
            games: self
                .tracked_apps
                .iter()
                .filter_map(|(app_id, tracked_app)| {
                    Some(RunningSteamGame {
                        app_id: *app_id,
                        launched_at: tracked_app.launched_at,
                        tracked_processes: live_tracked_processes(&tracked_app.tracked_process_ids),
                        installed_app: installed_app_in_libraries(library_folders, *app_id)?,
                    })
                })
                .collect(),
        }
    }
}

impl SnapshotSource for RunningSteamGames {
    type Error = ChangeWatchFailure;
    type StreamInputs = ();

    const NAME: &'static str = "running Steam games";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let followed = SteamRoot::located().map(|steam_root| {
            let library_folders =
                library_folder_paths(&steam_root).unwrap_or_else(|manifest_failure| {
                    tracing::debug!("{manifest_failure}");
                    Vec::new()
                });
            appended_log_lines(steam_root.log_file(GAME_PROCESS_LOG_FILE)).scan(
                GameProcessLogState::default(),
                move |log_state, appended| {
                    ready(Some(appended.map(|lines| {
                        log_state.absorb_lines(&lines);
                        log_state.running_games(&library_folders)
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

    fn stamped_lines(messages: &[&str]) -> Vec<String> {
        messages
            .iter()
            .map(|message| format!("[2026-11-18 19:14:16] {message}"))
            .collect()
    }

    #[test]
    fn a_game_runs_from_its_first_tracked_process_until_removed() {
        let mut log_state = GameProcessLogState::default();
        log_state.absorb_lines(&stamped_lines(&[
            "AppID 284160 adding PID 161873 as a tracked process",
            "AppID 284160 adding PID 161967 as a tracked process",
            "AppID 284160 no longer tracking PID 161967, exit code -1",
        ]));
        let tracked_app = &log_state.tracked_apps[&SteamAppId { number: 284_160 }];
        assert_eq!(tracked_app.tracked_process_ids, BTreeSet::from([161_873]));
        log_state.absorb_lines(&stamped_lines(&["Remove 284160 from running list"]));
        assert!(log_state.tracked_apps.is_empty());
    }

    #[test]
    fn a_steam_restart_forgets_games_left_running() {
        let mut log_state = GameProcessLogState::default();
        log_state.absorb_lines(&stamped_lines(&[
            "AppID 284160 adding PID 161873 as a tracked process",
            "Client version: 1794536281",
        ]));
        assert!(log_state.tracked_apps.is_empty());
    }
}
