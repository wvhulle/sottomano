use std::collections::BTreeMap;

use super::steam_log_lines::hardware_update_log_lines::{HardwareUpdateLogLine, SteamHardwareKind};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HardwareUpdateState {
    pending_by_serial: BTreeMap<String, SteamHardwareKind>,
}

impl HardwareUpdateState {
    pub fn absorb(&mut self, line: HardwareUpdateLogLine) {
        match line {
            HardwareUpdateLogLine::CheckStarted => self.pending_by_serial.clear(),
            HardwareUpdateLogLine::UpdateAvailable {
                hardware,
                serial_number,
            } => {
                self.pending_by_serial.insert(serial_number, hardware);
            }
            HardwareUpdateLogLine::UpdateCompleted { serial_number } => {
                self.pending_by_serial.remove(&serial_number);
            }
            HardwareUpdateLogLine::Unrecognised => {}
        }
    }

    #[must_use]
    pub fn pending_update_for(&self, serial_number: &str) -> Option<SteamHardwareKind> {
        self.pending_by_serial
            .iter()
            .find(|(pending_serial, _)| pending_serial.eq_ignore_ascii_case(serial_number))
            .map(|(_, hardware)| *hardware)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn available(serial_number: &str, hardware: SteamHardwareKind) -> HardwareUpdateLogLine {
        HardwareUpdateLogLine::UpdateAvailable {
            hardware,
            serial_number: serial_number.to_owned(),
        }
    }

    #[test]
    fn an_update_stays_pending_until_completed() {
        let mut state = HardwareUpdateState::default();
        state.absorb(HardwareUpdateLogLine::CheckStarted);
        state.absorb(available("FXB0000000001", SteamHardwareKind::PuckDongle));
        assert_eq!(
            state.pending_update_for("FXB0000000001"),
            Some(SteamHardwareKind::PuckDongle)
        );
        state.absorb(HardwareUpdateLogLine::UpdateCompleted {
            serial_number: "FXB0000000001".to_owned(),
        });
        assert_eq!(state.pending_update_for("FXB0000000001"), None);
    }

    #[test]
    fn a_new_check_forgets_updates_it_no_longer_reports() {
        let mut state = HardwareUpdateState::default();
        state.absorb(available(
            "FXA0000000002",
            SteamHardwareKind::ControllerOverRadio,
        ));
        state.absorb(HardwareUpdateLogLine::CheckStarted);
        assert_eq!(state.pending_update_for("FXA0000000002"), None);
    }
}
