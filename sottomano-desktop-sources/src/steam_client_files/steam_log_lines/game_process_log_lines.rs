use winnow::{
    Parser, Result,
    ascii::dec_uint,
    combinator::{alt, delimited, preceded, separated_pair},
    token::rest,
};

use super::{SteamAppId, app_id};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameProcessLogLine {
    TrackingStarted { app_id: SteamAppId, process_id: u32 },
    TrackingStopped { app_id: SteamAppId, process_id: u32 },
    RemovedFromRunning { app_id: SteamAppId },
    ClientStarted,
    Unrecognised,
}

fn tracking_started(input: &mut &str) -> Result<GameProcessLogLine> {
    (
        delimited("AppID ", app_id, " adding PID "),
        dec_uint,
        " as a tracked process",
    )
        .map(|(app_id, process_id, _)| GameProcessLogLine::TrackingStarted { app_id, process_id })
        .parse_next(input)
}

fn tracking_stopped(input: &mut &str) -> Result<GameProcessLogLine> {
    separated_pair(
        preceded("AppID ", app_id),
        " no longer tracking PID ",
        dec_uint,
    )
    .map(|(app_id, process_id)| GameProcessLogLine::TrackingStopped { app_id, process_id })
    .parse_next(input)
}

fn removed_from_running(input: &mut &str) -> Result<GameProcessLogLine> {
    delimited("Remove ", app_id, " from running list")
        .map(|app_id| GameProcessLogLine::RemovedFromRunning { app_id })
        .parse_next(input)
}

fn client_started(input: &mut &str) -> Result<GameProcessLogLine> {
    "Client version: "
        .value(GameProcessLogLine::ClientStarted)
        .parse_next(input)
}

impl From<&str> for GameProcessLogLine {
    fn from(mut message: &str) -> Self {
        let recognised = alt((
            tracking_started,
            tracking_stopped,
            removed_from_running,
            client_started,
            rest.value(GameProcessLogLine::Unrecognised),
        ))
        .parse_next(&mut message);
        match recognised {
            Ok(meaning) => meaning,
            Err(_) => GameProcessLogLine::Unrecognised,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracked_processes_come_and_go_per_app() {
        assert_eq!(
            GameProcessLogLine::from("AppID 284160 adding PID 161873 as a tracked process"),
            GameProcessLogLine::TrackingStarted {
                app_id: SteamAppId { number: 284_160 },
                process_id: 161_873
            }
        );
        assert_eq!(
            GameProcessLogLine::from("AppID 284160 no longer tracking PID 161968, exit code -1"),
            GameProcessLogLine::TrackingStopped {
                app_id: SteamAppId { number: 284_160 },
                process_id: 161_968
            }
        );
        assert_eq!(
            GameProcessLogLine::from("Remove 284160 from running list"),
            GameProcessLogLine::RemovedFromRunning {
                app_id: SteamAppId { number: 284_160 }
            }
        );
    }

    #[test]
    fn a_client_version_line_marks_a_steam_restart() {
        assert_eq!(
            GameProcessLogLine::from("Client version: 1794536281"),
            GameProcessLogLine::ClientStarted
        );
        assert_eq!(
            GameProcessLogLine::from("SSGL: UI mode (0->7)"),
            GameProcessLogLine::Unrecognised
        );
    }
}
