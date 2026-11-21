use std::{
    fs,
    future::{Ready, ready},
    io,
    path::PathBuf,
};

use futures::{Stream, StreamExt, stream};

use super::{
    appended_log_follow::appended_log_lines,
    hardware_update_state::HardwareUpdateState,
    steam_controller_log_state::SteamControllerLogState,
    steam_log_lines::{
        LogLineShape, controller_log_lines::ControllerLogLine,
        hardware_update_log_lines::HardwareUpdateLogLine,
    },
    steam_root_location::SteamRoot,
    virtual_gamepad_slots::{
        SteamVirtualSlot, VirtualSlotTableSyntaxError, parse_virtual_slot_table,
    },
};
use crate::{
    delivery_cadence_drivers::{ChangeTriggerStreams, ChangeWatchFailure},
    system_protocol_clients::inotify_directory_changes::{self, WatchTarget},
};

const VIRTUAL_SLOT_TABLE_FILE: &str = "virtualgamepadinfo.txt";
const CONTROLLER_LOG_FILE: &str = "controller.txt";
const HARDWARE_UPDATE_LOG_FILE: &str = "steamui_system.txt";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SteamControllerFacts {
    pub virtual_slots: Vec<SteamVirtualSlot>,
    pub controller_log: SteamControllerLogState,
    pub hardware_updates: HardwareUpdateState,
}

#[derive(Debug, thiserror::Error)]
pub enum SteamFileReadFailure {
    #[error(transparent)]
    Watch(#[from] ChangeWatchFailure),
    #[error("Steam file unreadable: {0}")]
    Read(#[from] io::Error),
    #[error(transparent)]
    SlotTable(#[from] VirtualSlotTableSyntaxError),
}

enum FactsChange {
    VirtualSlots(Vec<SteamVirtualSlot>),
    ControllerLogLines(Vec<String>),
    HardwareUpdateLogLines(Vec<String>),
}

impl SteamControllerFacts {
    fn apply(&mut self, change: FactsChange) {
        match change {
            FactsChange::VirtualSlots(virtual_slots) => self.virtual_slots = virtual_slots,
            FactsChange::ControllerLogLines(lines) => {
                for line in &lines {
                    self.controller_log
                        .absorb(ControllerLogLine::from(LogLineShape::from(line.as_str())));
                }
            }
            FactsChange::HardwareUpdateLogLines(lines) => {
                for line in &lines {
                    if let LogLineShape::Stamped { message, .. } = LogLineShape::from(line.as_str())
                    {
                        self.hardware_updates
                            .absorb(HardwareUpdateLogLine::from(message));
                    }
                }
            }
        }
    }
}

fn read_virtual_slot_table(
    table_path: &PathBuf,
) -> Result<Vec<SteamVirtualSlot>, SteamFileReadFailure> {
    match fs::read_to_string(table_path) {
        Ok(table_text) => Ok(parse_virtual_slot_table(&table_text)?),
        Err(read_failure) if read_failure.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(read_failure) => Err(read_failure.into()),
    }
}

fn logged_failures_skipped<Sample>(
    file_name: &'static str,
) -> impl FnMut(Result<Sample, SteamFileReadFailure>) -> Ready<Option<Sample>> {
    move |fetch| {
        ready(
            fetch
                .inspect_err(|read_failure| {
                    tracing::warn!("Steam {file_name}: {read_failure}");
                })
                .ok(),
        )
    }
}

fn followed_log(
    log_path: PathBuf,
    file_name: &'static str,
) -> impl Stream<Item = Vec<String>> + Send {
    appended_log_lines(log_path)
        .map(|fetch| fetch.map_err(SteamFileReadFailure::from))
        .filter_map(logged_failures_skipped(file_name))
}

pub fn steam_controller_facts(
    steam_root: &SteamRoot,
) -> impl Stream<Item = SteamControllerFacts> + Send + use<> {
    let table_path = steam_root.config_file(VIRTUAL_SLOT_TABLE_FILE);
    let slot_changes =
        inotify_directory_changes::changes(vec![WatchTarget::File(table_path.clone())])
            .triggered_rereads(move || ready(read_virtual_slot_table(&table_path)))
            .filter_map(logged_failures_skipped(VIRTUAL_SLOT_TABLE_FILE))
            .map(FactsChange::VirtualSlots);
    let controller_changes = followed_log(
        steam_root.log_file(CONTROLLER_LOG_FILE),
        CONTROLLER_LOG_FILE,
    )
    .map(FactsChange::ControllerLogLines);
    let hardware_changes = followed_log(
        steam_root.log_file(HARDWARE_UPDATE_LOG_FILE),
        HARDWARE_UPDATE_LOG_FILE,
    )
    .map(FactsChange::HardwareUpdateLogLines);
    stream::select_all([
        slot_changes.boxed(),
        controller_changes.boxed(),
        hardware_changes.boxed(),
    ])
    .scan(SteamControllerFacts::default(), |facts, change| {
        facts.apply(change);
        ready(Some(facts.clone()))
    })
}
