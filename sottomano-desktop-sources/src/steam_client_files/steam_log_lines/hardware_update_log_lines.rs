use winnow::{
    Parser, Result,
    combinator::{alt, delimited, preceded, separated_pair},
    token::{rest, take_till},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SteamHardwareKind {
    ControllerOverRadio,
    ControllerOverUsb,
    PuckDongle,
    OtherHardware,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HardwareUpdateLogLine {
    CheckStarted,
    UpdateAvailable {
        hardware: SteamHardwareKind,
        serial_number: String,
    },
    UpdateCompleted { serial_number: String },
    Unrecognised,
}

fn hardware_kind(input: &mut &str) -> Result<SteamHardwareKind> {
    take_till(1.., ' ')
        .map(|kind: &str| match kind {
            "Triton_ESB" => SteamHardwareKind::ControllerOverRadio,
            "Triton_USB" => SteamHardwareKind::ControllerOverUsb,
            "Proteus_USB" => SteamHardwareKind::PuckDongle,
            _ => SteamHardwareKind::OtherHardware,
        })
        .parse_next(input)
}

fn check_started(input: &mut &str) -> Result<HardwareUpdateLogLine> {
    "starting update check"
        .value(HardwareUpdateLogLine::CheckStarted)
        .parse_next(input)
}

fn update_available(input: &mut &str) -> Result<HardwareUpdateLogLine> {
    preceded(
        "update available: etype: ",
        separated_pair(
            hardware_kind,
            " serial_number: ",
            delimited('"', take_till(0.., '"'), '"'),
        ),
    )
    .map(|(hardware, serial_number): (SteamHardwareKind, &str)| {
        HardwareUpdateLogLine::UpdateAvailable {
            hardware,
            serial_number: serial_number.to_owned(),
        }
    })
    .parse_next(input)
}

fn update_completed(input: &mut &str) -> Result<HardwareUpdateLogLine> {
    preceded("update complete: ", take_till(1.., char::is_whitespace))
        .map(|serial_number: &str| HardwareUpdateLogLine::UpdateCompleted {
            serial_number: serial_number.to_owned(),
        })
        .parse_next(input)
}

impl From<&str> for HardwareUpdateLogLine {
    fn from(mut message: &str) -> Self {
        let recognised = preceded(
            "HardwareUpdate: ",
            alt((
                check_started,
                update_available,
                update_completed,
                rest.value(HardwareUpdateLogLine::Unrecognised),
            )),
        )
        .parse_next(&mut message);
        match recognised {
            Ok(meaning) => meaning,
            Err(_) => HardwareUpdateLogLine::Unrecognised,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn available_updates_name_the_hardware_and_its_serial() {
        assert_eq!(
            HardwareUpdateLogLine::from(
                "HardwareUpdate: update available: etype: Proteus_USB serial_number: \
                 \"FXB0000000001\" current_ts: 1 update_ts: 2 priority: Normal is_downgrade: false"
            ),
            HardwareUpdateLogLine::UpdateAvailable {
                hardware: SteamHardwareKind::PuckDongle,
                serial_number: "FXB0000000001".to_owned(),
            }
        );
    }

    #[test]
    fn checks_and_completions_frame_the_pending_set() {
        assert_eq!(
            HardwareUpdateLogLine::from("HardwareUpdate: starting update check"),
            HardwareUpdateLogLine::CheckStarted
        );
        assert_eq!(
            HardwareUpdateLogLine::from("HardwareUpdate: update complete: FXA0000000002"),
            HardwareUpdateLogLine::UpdateCompleted {
                serial_number: "FXA0000000002".to_owned()
            }
        );
        assert_eq!(
            HardwareUpdateLogLine::from("HardwareUpdate: ------[ stdout ]------"),
            HardwareUpdateLogLine::Unrecognised
        );
    }
}
