use std::time::Duration;

use winnow::{
    Parser, Result,
    ascii::{dec_uint, float, space0},
    combinator::{alt, delimited, opt, preceded, repeat, separated_pair, terminated},
    error::ContextError,
    token::{rest, take_till},
};

use super::{SteamAppId, app_id};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContentStateFlag {
    FullyInstalled,
    UpdateRequired,
    UpdateQueued,
    UpdateRunning,
    UpdateStarted,
    UpdateOptional,
    AppRunning,
    FilesCorrupt,
    OtherFlag(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateHold {
    Unheld,
    Suspended,
    DelayedFor(Duration),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpdateProgress {
    pub downloaded_bytes: u64,
    pub download_total_bytes: u64,
    pub staged_bytes: u64,
    pub stage_total_bytes: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ContentLogLine {
    StateChanged {
        app_id: SteamAppId,
        flags: Vec<ContentStateFlag>,
        hold: UpdateHold,
    },
    UpdateStarted {
        app_id: SteamAppId,
        progress: UpdateProgress,
    },
    UpdateFinished {
        app_id: SteamAppId,
    },
    UpdateCanceled {
        app_id: SteamAppId,
    },
    DownloadRate {
        megabits_per_second: f64,
    },
    ClientStarted,
    Unrecognised,
}

fn state_flag(input: &mut &str) -> Result<ContentStateFlag> {
    terminated(take_till(1.., [',', '(']), ',')
        .map(|flag: &str| match flag.trim() {
            "Fully Installed" => ContentStateFlag::FullyInstalled,
            "Update Required" => ContentStateFlag::UpdateRequired,
            "Update Queued" => ContentStateFlag::UpdateQueued,
            "Update Running" => ContentStateFlag::UpdateRunning,
            "Update Started" => ContentStateFlag::UpdateStarted,
            "Update Optional" => ContentStateFlag::UpdateOptional,
            "App Running" => ContentStateFlag::AppRunning,
            "Files Corrupt" => ContentStateFlag::FilesCorrupt,
            other => ContentStateFlag::OtherFlag(other.to_owned()),
        })
        .parse_next(input)
}

fn update_hold(input: &mut &str) -> Result<UpdateHold> {
    preceded(
        space0,
        opt(alt((
            "(Suspended)".value(UpdateHold::Suspended),
            delimited("(Update delayed for ", dec_uint, " secs)")
                .map(|seconds| UpdateHold::DelayedFor(Duration::from_secs(seconds))),
        ))),
    )
    .map(|hold| hold.unwrap_or(UpdateHold::Unheld))
    .parse_next(input)
}

fn state_changed(input: &mut &str) -> Result<ContentLogLine> {
    (
        delimited("AppID ", app_id, " state changed : "),
        repeat(0.., state_flag),
        update_hold,
    )
        .map(|(app_id, flags, hold)| ContentLogLine::StateChanged {
            app_id,
            flags,
            hold,
        })
        .parse_next(input)
}

fn byte_fraction<'input>(
    label: &'static str,
) -> impl Parser<&'input str, (u64, u64), ContextError> {
    preceded((label, ' '), separated_pair(dec_uint, '/', dec_uint))
}

fn update_started(input: &mut &str) -> Result<ContentLogLine> {
    (
        delimited("AppID ", app_id, " update started : "),
        terminated(byte_fraction("download"), ", "),
        terminated(byte_fraction("store"), ", "),
        terminated(byte_fraction("reuse"), ", "),
        terminated(byte_fraction("delta"), ", "),
        byte_fraction("stage"),
    )
        .map(
            |(
                app_id,
                (downloaded_bytes, download_total_bytes),
                _,
                _,
                _,
                (staged_bytes, stage_total_bytes),
            )| {
                ContentLogLine::UpdateStarted {
                    app_id,
                    progress: UpdateProgress {
                        downloaded_bytes,
                        download_total_bytes,
                        staged_bytes,
                        stage_total_bytes,
                    },
                }
            },
        )
        .parse_next(input)
}

fn update_finished(input: &mut &str) -> Result<ContentLogLine> {
    delimited("AppID ", app_id, " finished update")
        .map(|app_id| ContentLogLine::UpdateFinished { app_id })
        .parse_next(input)
}

fn update_canceled(input: &mut &str) -> Result<ContentLogLine> {
    delimited("AppID ", app_id, " update canceled")
        .map(|app_id| ContentLogLine::UpdateCanceled { app_id })
        .parse_next(input)
}

fn download_rate(input: &mut &str) -> Result<ContentLogLine> {
    delimited("Current download rate: ", float, " Mbps")
        .map(|megabits_per_second| ContentLogLine::DownloadRate {
            megabits_per_second,
        })
        .parse_next(input)
}

fn client_started(input: &mut &str) -> Result<ContentLogLine> {
    "Client version: "
        .value(ContentLogLine::ClientStarted)
        .parse_next(input)
}

impl From<&str> for ContentLogLine {
    fn from(mut message: &str) -> Self {
        let recognised = alt((
            state_changed,
            update_started,
            update_finished,
            update_canceled,
            download_rate,
            client_started,
            rest.value(ContentLogLine::Unrecognised),
        ))
        .parse_next(&mut message);
        match recognised {
            Ok(meaning) => meaning,
            Err(_) => ContentLogLine::Unrecognised,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_changes_list_flags_and_a_delay() {
        assert_eq!(
            ContentLogLine::from(
                "AppID 1478970 state changed : Update Required,Fully Installed, (Update delayed \
                 for 59703 secs)"
            ),
            ContentLogLine::StateChanged {
                app_id: SteamAppId { number: 1_478_970 },
                flags: vec![
                    ContentStateFlag::UpdateRequired,
                    ContentStateFlag::FullyInstalled
                ],
                hold: UpdateHold::DelayedFor(Duration::from_secs(59_703)),
            }
        );
    }

    #[test]
    fn suspended_updates_are_held() {
        assert_eq!(
            ContentLogLine::from(
                "AppID 228980 state changed : Update Required,Fully Installed,Update \
                 Queued,Update Started, (Suspended)"
            ),
            ContentLogLine::StateChanged {
                app_id: SteamAppId { number: 228_980 },
                flags: vec![
                    ContentStateFlag::UpdateRequired,
                    ContentStateFlag::FullyInstalled,
                    ContentStateFlag::UpdateQueued,
                    ContentStateFlag::UpdateStarted,
                ],
                hold: UpdateHold::Suspended,
            }
        );
    }

    #[test]
    fn update_progress_reports_download_and_stage_bytes() {
        assert_eq!(
            ContentLogLine::from(
                "AppID 228980 update started : download 400/678, store 0/0, reuse 242/242, delta \
                 0/0, stage 100/697 "
            ),
            ContentLogLine::UpdateStarted {
                app_id: SteamAppId { number: 228_980 },
                progress: UpdateProgress {
                    downloaded_bytes: 400,
                    download_total_bytes: 678,
                    staged_bytes: 100,
                    stage_total_bytes: 697,
                },
            }
        );
    }

    #[test]
    fn rates_and_completions_are_recognised() {
        assert_eq!(
            ContentLogLine::from("Current download rate: 12.500 Mbps"),
            ContentLogLine::DownloadRate {
                megabits_per_second: 12.5
            }
        );
        assert_eq!(
            ContentLogLine::from(
                "AppID 275850 finished update, 1 mounted depots (BuildID 1) : 1 (1),"
            ),
            ContentLogLine::UpdateFinished {
                app_id: SteamAppId { number: 275_850 }
            }
        );
        assert_eq!(
            ContentLogLine::from("AppID 228980 update canceled : Priority (Suspended)"),
            ContentLogLine::UpdateCanceled {
                app_id: SteamAppId { number: 228_980 }
            }
        );
    }
}
